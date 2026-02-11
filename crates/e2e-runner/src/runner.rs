//! Test scenario runner.

use crate::metrics::{calculate_metrics, check_thresholds, TestMetrics};
use crate::truth::load_truth;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::Duration;
use tracing::{info, warn};

/// Configuration for running a scenario.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub scenario_path: PathBuf,
    pub artifacts_path: PathBuf,
    pub keep_running: bool,
    #[allow(dead_code)]
    pub timeout_seconds: u64,
}

/// Result of running a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub scenario_name: String,
    pub passed: bool,
    pub metrics: TestMetrics,
    pub failures: Vec<String>,
    pub bundle_path: Option<PathBuf>,
    pub plan_path: Option<PathBuf>,
    pub duration_seconds: f64,
}

/// Run a test scenario.
pub async fn run_scenario(config: &RunConfig) -> Result<RunResult> {
    let start = std::time::Instant::now();

    // Load truth file
    let truth_path = config.scenario_path.join("truth.json");
    let truth = load_truth(&truth_path).context("Failed to load truth.json")?;

    let scenario_name = truth.name.clone();
    info!("Running scenario: {}", scenario_name);

    // Create artifacts directory
    std::fs::create_dir_all(&config.artifacts_path)?;

    // Step 1: Start docker-compose
    info!("Starting docker-compose...");
    let compose_file = if config.scenario_path.join("compose.yaml").exists() {
        "compose.yaml"
    } else if config.scenario_path.join("docker-compose.yaml").exists() {
        "docker-compose.yaml"
    } else {
        anyhow::bail!("No compose.yaml or docker-compose.yaml found in scenario");
    };

    let compose_up = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file,
            "up",
            "-d",
            "--build",
            "--wait",
        ])
        .current_dir(&config.scenario_path)
        .output()
        .context("Failed to run docker compose up")?;

    if !compose_up.status.success() {
        let stderr = String::from_utf8_lossy(&compose_up.stderr);
        anyhow::bail!("docker compose up failed: {}", stderr);
    }

    // Step 2: Wait for services to be ready (--wait flag handles healthchecks,
    // add a small grace period for processes to fully initialize)
    info!("Waiting for services to stabilize...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Step 3: Run xcprobe collect
    info!("Running xcprobe collect...");
    let bundle_path = config.artifacts_path.join("bundle.tgz");

    let collect_result = run_collect(&config.scenario_path, &bundle_path).await;

    let bundle_path = match collect_result {
        Ok(path) => Some(path),
        Err(e) => {
            warn!("xcprobe collect failed: {}", e);
            None
        }
    };

    // Step 4: Run xcprobe analyze
    let plan_path = if let Some(ref bundle) = bundle_path {
        info!("Running xcprobe analyze...");
        let plan_path = config.artifacts_path.join("packplan.json");
        let analyze_result = run_analyze(bundle, &plan_path).await;

        match analyze_result {
            Ok(path) => Some(path),
            Err(e) => {
                warn!("xcprobe analyze failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Step 5: Compare results to truth
    let (metrics, failures) = if let Some(ref plan) = plan_path {
        let plan_content = std::fs::read_to_string(plan)?;
        let pack_plan: xcprobe_bundle_schema::PackPlan = serde_json::from_str(&plan_content)?;

        let metrics = calculate_metrics(&pack_plan, &truth);
        let failures = check_thresholds(&metrics, &truth);

        (metrics, failures)
    } else {
        // No plan = all failures
        let mut failures = vec!["Failed to generate pack plan".to_string()];
        let metrics = TestMetrics::default();
        failures.extend(check_thresholds(&metrics, &truth));
        (metrics, failures)
    };

    // Step 6: Cleanup (unless keep_running)
    if !config.keep_running {
        info!("Stopping docker-compose...");
        let _ = Command::new("docker")
            .args(["compose", "-f", compose_file, "down", "-v"])
            .current_dir(&config.scenario_path)
            .output();
    }

    // Step 7: Archive artifacts if failed
    let passed = failures.is_empty();
    if !passed {
        info!("Test failed, archiving artifacts...");
        archive_artifacts(config, &bundle_path, &plan_path)?;
    }

    let duration = start.elapsed();

    // Write report
    let result = RunResult {
        scenario_name,
        passed,
        metrics,
        failures,
        bundle_path,
        plan_path,
        duration_seconds: duration.as_secs_f64(),
    };

    let report_path = config.artifacts_path.join("report.json");
    let report_json = serde_json::to_string_pretty(&result)?;
    std::fs::write(&report_path, report_json)?;

    Ok(result)
}

async fn run_collect(scenario_path: &Path, bundle_path: &Path) -> Result<PathBuf> {
    let compose_file = if scenario_path.join("compose.yaml").exists() {
        "compose.yaml"
    } else {
        "docker-compose.yaml"
    };

    // Find xcprobe binary: check PATH, then common build output paths
    let xcprobe_path =
        find_binary("xcprobe").context("xcprobe binary not found in PATH or target/ directory")?;

    info!("Copying xcprobe to container from {:?}", xcprobe_path);

    // Copy xcprobe into the container
    let copy_binary = Command::new("docker")
        .args(["compose", "-f", compose_file, "cp"])
        .arg(&xcprobe_path)
        .arg("host-sim:/xcprobe")
        .current_dir(scenario_path)
        .output()
        .context("Failed to copy xcprobe to container")?;

    if !copy_binary.status.success() {
        let stderr = String::from_utf8_lossy(&copy_binary.stderr);
        anyhow::bail!("Failed to copy xcprobe to container: {}", stderr);
    }

    // Make it executable
    let chmod = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file,
            "exec",
            "-T",
            "host-sim",
            "chmod",
            "+x",
            "/xcprobe",
        ])
        .current_dir(scenario_path)
        .output()
        .context("Failed to chmod xcprobe")?;

    if !chmod.status.success() {
        let stderr = String::from_utf8_lossy(&chmod.stderr);
        anyhow::bail!("Failed to chmod xcprobe: {}", stderr);
    }

    // Run xcprobe collect inside the host-sim container
    let output = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file,
            "exec",
            "-T",
            "host-sim",
            "/xcprobe",
            "collect",
            "--target",
            "localhost",
            "--os",
            "linux",
            "--mode",
            "local-ephemeral",
            "--out",
            "/tmp/bundle.tgz",
        ])
        .current_dir(scenario_path)
        .output()
        .context("Failed to run xcprobe collect")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("xcprobe collect failed: {}", stderr);
    }

    // Copy bundle out of container
    let copy_output = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file,
            "cp",
            "host-sim:/tmp/bundle.tgz",
        ])
        .arg(bundle_path)
        .current_dir(scenario_path)
        .output()
        .context("Failed to copy bundle from container")?;

    if !copy_output.status.success() {
        let stderr = String::from_utf8_lossy(&copy_output.stderr);
        anyhow::bail!("Failed to copy bundle: {}", stderr);
    }

    Ok(bundle_path.to_path_buf())
}

async fn run_analyze(bundle_path: &Path, plan_path: &Path) -> Result<PathBuf> {
    let output_dir = plan_path.parent().unwrap();

    // Find xcprobe binary: check PATH, then common build output paths
    let xcprobe_path =
        find_binary("xcprobe").context("xcprobe binary not found in PATH or target/ directory")?;

    let output = Command::new(&xcprobe_path)
        .args(["analyze", "--bundle"])
        .arg(bundle_path)
        .args(["--out"])
        .arg(output_dir)
        .output()
        .context("Failed to run xcprobe analyze")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("xcprobe analyze failed: {}", stderr);
    }

    Ok(plan_path.to_path_buf())
}

/// Find a binary by name, checking PATH first, then target/release and target/debug.
fn find_binary(name: &str) -> Result<PathBuf> {
    // Check PATH
    if let Ok(path) = which::which(name) {
        return Ok(path);
    }

    // Check target/release/
    if let Ok(cwd) = std::env::current_dir() {
        let release_path = cwd.join("target/release").join(name);
        if release_path.exists() {
            return Ok(release_path);
        }

        let debug_path = cwd.join("target/debug").join(name);
        if debug_path.exists() {
            return Ok(debug_path);
        }
    }

    anyhow::bail!("{} not found in PATH or target/ directory", name)
}

fn archive_artifacts(
    config: &RunConfig,
    bundle_path: &Option<PathBuf>,
    plan_path: &Option<PathBuf>,
) -> Result<()> {
    let archive_dir = config.artifacts_path.join("failed_artifacts");
    std::fs::create_dir_all(&archive_dir)?;

    if let Some(bundle) = bundle_path {
        if bundle.exists() {
            std::fs::copy(bundle, archive_dir.join("bundle.tgz"))?;
        }
    }

    if let Some(plan) = plan_path {
        if plan.exists() {
            std::fs::copy(plan, archive_dir.join("packplan.json"))?;
        }
    }

    Ok(())
}

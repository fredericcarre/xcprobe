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
    let compose_path = config.scenario_path.join("compose.yaml");
    if !compose_path.exists() {
        // Try docker-compose.yaml
        let alt_path = config.scenario_path.join("docker-compose.yaml");
        if !alt_path.exists() {
            anyhow::bail!("No compose.yaml or docker-compose.yaml found in scenario");
        }
    }

    let compose_up = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_path)
        .args(["up", "-d"])
        .current_dir(&config.scenario_path)
        .output()
        .context("Failed to run docker compose up")?;

    if !compose_up.status.success() {
        let stderr = String::from_utf8_lossy(&compose_up.stderr);
        anyhow::bail!("docker compose up failed: {}", stderr);
    }

    // Step 2: Wait for services to be ready
    info!("Waiting for services to be ready...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Step 3: Run probe-cli collect
    info!("Running probe-cli collect...");
    let bundle_path = config.artifacts_path.join("bundle.tgz");

    let collect_result = run_probe_collect(&config.scenario_path, &bundle_path).await;

    let bundle_path = match collect_result {
        Ok(path) => Some(path),
        Err(e) => {
            warn!("probe-cli collect failed: {}", e);
            None
        }
    };

    // Step 4: Run analyzer
    let plan_path = if let Some(ref bundle) = bundle_path {
        info!("Running analyzer...");
        let plan_path = config.artifacts_path.join("packplan.json");
        let analyze_result = run_analyzer(bundle, &plan_path).await;

        match analyze_result {
            Ok(path) => Some(path),
            Err(e) => {
                warn!("analyzer failed: {}", e);
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
            .args(["compose", "-f"])
            .arg(&compose_path)
            .args(["down", "-v"])
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

async fn run_probe_collect(scenario_path: &Path, bundle_path: &Path) -> Result<PathBuf> {
    // Get the host-sim container name
    let compose_path = scenario_path.join("compose.yaml");

    // Run probe-cli inside the host-sim container
    let output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_path)
        .args([
            "exec",
            "-T",
            "host-sim",
            "/probe-cli",
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
        .context("Failed to run probe-cli")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("probe-cli collect failed: {}", stderr);
    }

    // Copy bundle out of container
    let copy_output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_path)
        .args(["cp", "host-sim:/tmp/bundle.tgz"])
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

async fn run_analyzer(bundle_path: &Path, plan_path: &Path) -> Result<PathBuf> {
    let output_dir = plan_path.parent().unwrap();

    let output = Command::new("analyzer")
        .args(["analyze", "--bundle"])
        .arg(bundle_path)
        .args(["--out"])
        .arg(output_dir)
        .output()
        .context("Failed to run analyzer")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("analyzer failed: {}", stderr);
    }

    Ok(plan_path.to_path_buf())
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

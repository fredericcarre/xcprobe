//! Test scenario runner.

use crate::metrics::{calculate_metrics, check_thresholds, TestMetrics};
use crate::truth::load_truth;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::time::Duration;
use tracing::{debug, info, warn};

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

/// Resolve a path to absolute (relative to cwd).
fn to_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        Ok(cwd.join(path))
    }
}

/// Find the compose file in a scenario directory and return its absolute path.
fn find_compose_file(scenario_path: &Path) -> Result<PathBuf> {
    let yaml = scenario_path.join("compose.yaml");
    if yaml.exists() {
        return Ok(yaml);
    }
    let docker_yaml = scenario_path.join("docker-compose.yaml");
    if docker_yaml.exists() {
        return Ok(docker_yaml);
    }
    anyhow::bail!("No compose.yaml or docker-compose.yaml found in scenario");
}

/// Run a test scenario.
pub async fn run_scenario(config: &RunConfig) -> Result<RunResult> {
    let start = std::time::Instant::now();

    // Resolve ALL paths to absolute up front so that docker compose commands
    // (which may change working directory) always see correct paths.
    let scenario_path = to_absolute(&config.scenario_path)?;
    let artifacts_path = to_absolute(&config.artifacts_path)?;
    let compose_file = find_compose_file(&scenario_path)?;

    // Load truth file
    let truth_path = scenario_path.join("truth.json");
    let truth = load_truth(&truth_path).context("Failed to load truth.json")?;

    let scenario_name = truth.name.clone();
    info!("Running scenario: {}", scenario_name);

    // Create artifacts directory
    std::fs::create_dir_all(&artifacts_path)?;

    // Step 1: Build images first (separate from up to get clearer errors)
    info!("Building docker images...");
    let compose_build = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_file)
        .args(["build"])
        .output()
        .context("Failed to run docker compose build")?;

    if !compose_build.status.success() {
        let stdout = String::from_utf8_lossy(&compose_build.stdout);
        let stderr = String::from_utf8_lossy(&compose_build.stderr);
        anyhow::bail!(
            "docker compose build failed:\nstdout: {}\nstderr: {}",
            stdout,
            stderr
        );
    }

    // Step 1b: Start services
    info!("Starting docker-compose services...");
    let compose_up = Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose_file)
        .args(["up", "-d", "--wait"])
        .output()
        .context("Failed to run docker compose up")?;

    if !compose_up.status.success() {
        let stdout = String::from_utf8_lossy(&compose_up.stdout);
        let stderr = String::from_utf8_lossy(&compose_up.stderr);
        // Grab container logs for debugging
        let logs = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["logs", "--tail", "50"])
            .output();
        let container_logs = logs
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        anyhow::bail!(
            "docker compose up failed:\nstdout: {}\nstderr: {}\ncontainer logs:\n{}",
            stdout,
            stderr,
            container_logs
        );
    }

    // Step 2: Wait for services to be ready (--wait flag handles healthchecks,
    // add a small grace period for processes to fully initialize)
    info!("Waiting for services to stabilize...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Step 3: Run xcprobe collect
    info!("Running xcprobe collect...");
    let bundle_path = artifacts_path.join("bundle.tgz");

    let collect_result = run_collect(&compose_file, &bundle_path).await;

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
        let plan_path = artifacts_path.join("packplan.json");
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
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["down", "-v"])
            .output();
    }

    // Step 7: Archive artifacts if failed
    let passed = failures.is_empty();
    if !passed {
        info!("Test failed, archiving artifacts...");
        archive_artifacts(&artifacts_path, &bundle_path, &plan_path)?;
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

    let report_path = artifacts_path.join("report.json");
    let report_json = serde_json::to_string_pretty(&result)?;
    std::fs::write(&report_path, report_json)?;

    Ok(result)
}

/// Run xcprobe collect inside the host-sim container.
/// `compose_file` must be an absolute path to the compose file.
/// `bundle_path` must be an absolute path for the output bundle.
async fn run_collect(compose_file: &Path, bundle_path: &Path) -> Result<PathBuf> {
    // Find xcprobe binary: check PATH, then common build output paths
    let xcprobe_path =
        find_binary("xcprobe").context("xcprobe binary not found in PATH or target/ directory")?;

    info!("Copying xcprobe to container from {:?}", xcprobe_path);

    // Copy xcprobe into the container
    let copy_binary = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .arg("cp")
        .arg(&xcprobe_path)
        .arg("host-sim:/xcprobe")
        .output()
        .context("Failed to copy xcprobe to container")?;

    if !copy_binary.status.success() {
        let stderr = String::from_utf8_lossy(&copy_binary.stderr);
        anyhow::bail!("Failed to copy xcprobe to container: {}", stderr);
    }

    // Make it executable
    let chmod = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args(["exec", "-T", "--user", "root", "host-sim", "chmod", "+x", "/xcprobe"])
        .output()
        .context("Failed to chmod xcprobe")?;

    if !chmod.status.success() {
        let stderr = String::from_utf8_lossy(&chmod.stderr);
        anyhow::bail!("Failed to chmod xcprobe: {}", stderr);
    }

    // Verify the binary can execute (check for missing shared libraries)
    let ldd_check = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args([
            "exec",
            "-T",
            "--user",
            "root",
            "host-sim",
            "sh",
            "-c",
            "ldd /xcprobe 2>&1 || echo 'ldd not available'",
        ])
        .output();
    if let Ok(ldd_out) = ldd_check {
        let ldd_stdout = String::from_utf8_lossy(&ldd_out.stdout);
        if ldd_stdout.contains("not found") {
            warn!("Missing shared libraries in container:\n{}", ldd_stdout);
        } else {
            debug!("Binary library check:\n{}", ldd_stdout);
        }
    }

    // Run xcprobe collect inside the host-sim container
    let output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args([
            "exec",
            "-T",
            "--user",
            "root",
            "host-sim",
            "/xcprobe",
            "collect",
            "--verbose",
            "--target",
            "localhost",
            "--os",
            "linux",
            "--mode",
            "local-ephemeral",
            "--out",
            "/tmp/bundle.tgz",
        ])
        .output()
        .context("Failed to run xcprobe collect")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            warn!("xcprobe collect stdout:\n{}", stdout);
        }
        anyhow::bail!(
            "xcprobe collect failed (exit {}): {}",
            output.status,
            stderr
        );
    }

    // Copy bundle out of container to local filesystem
    let copy_output = Command::new("docker")
        .args(["compose", "-f"])
        .arg(compose_file)
        .args(["cp", "host-sim:/tmp/bundle.tgz"])
        .arg(bundle_path)
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
        .args(["analyze", "--verbose", "--bundle"])
        .arg(bundle_path)
        .args(["--out"])
        .arg(output_dir)
        .args(["--min-confidence", "0.3"])
        .output()
        .context("Failed to run xcprobe analyze")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            warn!("xcprobe analyze stdout:\n{}", stdout);
        }
        anyhow::bail!(
            "xcprobe analyze failed (exit {}): {}",
            output.status,
            stderr
        );
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
    artifacts_path: &Path,
    bundle_path: &Option<PathBuf>,
    plan_path: &Option<PathBuf>,
) -> Result<()> {
    let archive_dir = artifacts_path.join("failed_artifacts");
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

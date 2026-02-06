//! XCProbe E2E Test Runner
//!
//! Runs end-to-end test scenarios and validates results against truth files.

mod metrics;
mod report;
mod runner;
mod truth;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "e2e-runner")]
#[command(author, version, about = "End-to-end test runner for xcprobe")]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single test scenario
    Run {
        /// Path to scenario directory
        #[arg(long)]
        scenario: PathBuf,

        /// Output directory for artifacts
        #[arg(long, default_value = "./artifacts")]
        artifacts: PathBuf,

        /// Keep containers running after test
        #[arg(long)]
        keep_running: bool,

        /// Timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,
    },

    /// Run all test scenarios in a directory
    RunAll {
        /// Directory containing scenarios
        #[arg(long, default_value = "./tests/scenarios")]
        scenarios_dir: PathBuf,

        /// Output directory for artifacts
        #[arg(long, default_value = "./artifacts")]
        artifacts: PathBuf,

        /// Continue on failure
        #[arg(long)]
        continue_on_failure: bool,
    },

    /// Validate a truth.json file
    ValidateTruth {
        /// Path to truth.json
        #[arg(long)]
        truth: PathBuf,
    },

    /// Generate a report from test results
    Report {
        /// Directory containing test results
        #[arg(long)]
        results: PathBuf,

        /// Output format (text, json, html)
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    match cli.command {
        Commands::Run {
            scenario,
            artifacts,
            keep_running,
            timeout,
        } => {
            info!("Running scenario: {:?}", scenario);

            let config = runner::RunConfig {
                scenario_path: scenario,
                artifacts_path: artifacts,
                keep_running,
                timeout_seconds: timeout,
            };

            let result = runner::run_scenario(&config).await?;

            // Print summary
            println!("\n=== Test Results ===");
            println!("Scenario: {}", result.scenario_name);
            println!(
                "Status: {}",
                if result.passed { "PASSED" } else { "FAILED" }
            );
            println!();

            // Print metrics
            println!("Metrics:");
            println!(
                "  Process/Cmdline Recall: {:.2}%",
                result.metrics.process_cmdline_recall * 100.0
            );
            println!(
                "  Ports Recall: {:.2}%",
                result.metrics.ports_recall * 100.0
            );
            println!(
                "  Env Names Recall: {:.2}%",
                result.metrics.env_names_recall * 100.0
            );
            println!(
                "  Dependencies Recall: {:.2}%",
                result.metrics.deps_recall * 100.0
            );
            println!(
                "  Decisions with Evidence: {:.2}%",
                result.metrics.decisions_with_evidence_ratio * 100.0
            );

            if !result.passed {
                println!("\nFailures:");
                for failure in &result.failures {
                    println!("  - {}", failure);
                }
                std::process::exit(1);
            }
        }

        Commands::RunAll {
            scenarios_dir,
            artifacts,
            continue_on_failure,
        } => {
            info!("Running all scenarios in: {:?}", scenarios_dir);

            let mut results = Vec::new();
            let mut all_passed = true;

            for entry in std::fs::read_dir(&scenarios_dir)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    let scenario_path = entry.path();
                    let truth_path = scenario_path.join("truth.json");

                    if truth_path.exists() {
                        info!("Running scenario: {:?}", scenario_path);

                        let config = runner::RunConfig {
                            scenario_path: scenario_path.clone(),
                            artifacts_path: artifacts.join(entry.file_name()),
                            keep_running: false,
                            timeout_seconds: 300,
                        };

                        match runner::run_scenario(&config).await {
                            Ok(result) => {
                                if !result.passed {
                                    all_passed = false;
                                }
                                results.push(result);
                            }
                            Err(e) => {
                                tracing::error!("Scenario {:?} failed: {}", scenario_path, e);
                                all_passed = false;
                                if !continue_on_failure {
                                    return Err(e);
                                }
                            }
                        }
                    }
                }
            }

            // Print summary
            println!("\n=== All Scenarios Summary ===");
            let passed_count = results.iter().filter(|r| r.passed).count();
            println!("Passed: {}/{}", passed_count, results.len());

            if !all_passed {
                std::process::exit(1);
            }
        }

        Commands::ValidateTruth { truth } => {
            info!("Validating truth file: {:?}", truth);

            let truth_data = truth::load_truth(&truth)?;
            let validation = truth::validate_truth(&truth_data);

            if validation.errors.is_empty() {
                println!("Truth file is valid");
            } else {
                println!("Truth file has errors:");
                for error in &validation.errors {
                    println!("  - {}", error);
                }
                std::process::exit(1);
            }
        }

        Commands::Report { results, format } => {
            info!("Generating report from: {:?}", results);
            report::generate_report(&results, &format)?;
        }
    }

    Ok(())
}

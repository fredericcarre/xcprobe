//! XCProbe Analyzer - Analyze bundles and generate Docker artifacts.

mod clustering;
mod confidence;
mod dependencies;
mod docker;
mod scoring;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "analyzer")]
#[command(author, version, about = "Analyze collection bundles and generate Docker artifacts")]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a bundle and generate artifacts
    Analyze {
        /// Input bundle file path
        #[arg(long)]
        bundle: PathBuf,

        /// Output directory for artifacts
        #[arg(long, short)]
        out: PathBuf,

        /// Cluster name prefix
        #[arg(long, default_value = "app")]
        cluster_prefix: String,

        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long, default_value = "0.7")]
        min_confidence: f64,
    },

    /// Validate a packplan for evidence-based decisions
    ValidatePlan {
        /// Pack plan file path
        #[arg(long)]
        plan: PathBuf,

        /// Fail if any decision lacks evidence
        #[arg(long)]
        strict: bool,
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
        Commands::Analyze {
            bundle,
            out,
            cluster_prefix,
            min_confidence,
        } => {
            info!("Analyzing bundle: {:?}", bundle);

            // Read bundle
            let bundle_data = read_bundle(&bundle)?;

            // Run analysis pipeline
            let pack_plan = analyze_bundle(&bundle_data, &cluster_prefix, min_confidence)?;

            // Generate artifacts
            std::fs::create_dir_all(&out)?;
            generate_artifacts(&pack_plan, &out)?;

            // Write pack plan
            let plan_path = out.join("packplan.json");
            let plan_json = serde_json::to_string_pretty(&pack_plan)?;
            std::fs::write(&plan_path, plan_json)?;

            info!("Analysis complete. Artifacts written to {:?}", out);
        }

        Commands::ValidatePlan { plan, strict } => {
            info!("Validating pack plan: {:?}", plan);

            let content = std::fs::read_to_string(&plan)?;
            let pack_plan: xcprobe_bundle_schema::PackPlan = serde_json::from_str(&content)?;

            let result = confidence::validate_plan_evidence(&pack_plan);

            if result.decisions_without_evidence.is_empty() {
                println!("All decisions have evidence references.");
            } else {
                println!("Decisions without evidence:");
                for decision in &result.decisions_without_evidence {
                    println!("  - {}", decision);
                }

                if strict {
                    anyhow::bail!("Strict mode: {} decisions lack evidence", result.decisions_without_evidence.len());
                }
            }

            println!("Overall confidence: {:.2}", result.overall_confidence);
        }
    }

    Ok(())
}

fn read_bundle(path: &PathBuf) -> Result<xcprobe_bundle_schema::Bundle> {
    use flate2::read::GzDecoder;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Read;
    use tar::Archive;

    let file = File::open(path).context("Failed to open bundle file")?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut manifest = None;
    let mut audit = Vec::new();
    let mut evidence = HashMap::new();
    let mut checksums = HashMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_string_lossy().to_string();

        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        if entry_path == "manifest.json" {
            manifest = Some(serde_json::from_slice(&content)?);
        } else if entry_path == "audit.jsonl" {
            let content_str = String::from_utf8_lossy(&content);
            for line in content_str.lines() {
                if !line.trim().is_empty() {
                    if let Ok(entry) = serde_json::from_str(line) {
                        audit.push(entry);
                    }
                }
            }
        } else if entry_path == "checksums.json" {
            checksums = serde_json::from_slice(&content)?;
        } else if entry_path.starts_with("evidence/") || entry_path.starts_with("attachments/") {
            let hash = xcprobe_common::hash::sha256_bytes(&content);
            let ev = xcprobe_bundle_schema::Evidence {
                id: entry_path.clone(),
                evidence_type: xcprobe_bundle_schema::EvidenceType::CommandOutput,
                collected_at: chrono::Utc::now(),
                source_command: None,
                size_bytes: content.len() as u64,
                content_hash: hash,
                redacted: false,
                bundle_path: entry_path.clone(),
                original_path: None,
                content: Some(content),
            };
            evidence.insert(entry_path, ev);
        }
    }

    Ok(xcprobe_bundle_schema::Bundle {
        manifest: manifest.context("Missing manifest.json in bundle")?,
        audit,
        evidence,
        checksums,
    })
}

fn analyze_bundle(
    bundle: &xcprobe_bundle_schema::Bundle,
    cluster_prefix: &str,
    min_confidence: f64,
) -> Result<xcprobe_bundle_schema::PackPlan> {
    // Step 1: Score processes/services for business relevance
    let scores = scoring::score_processes(&bundle.manifest);

    // Step 2: Cluster into applications
    let mut clusters = clustering::cluster_applications(bundle, &scores, cluster_prefix)?;

    // Step 3: Detect dependencies
    dependencies::detect_dependencies(bundle, &mut clusters)?;

    // Step 4: Build startup DAG
    let dag = dependencies::build_startup_dag(&clusters);

    // Step 5: Calculate confidence scores
    for cluster in &mut clusters {
        confidence::calculate_cluster_confidence(cluster);
    }

    // Filter by minimum confidence
    clusters.retain(|c| c.confidence >= min_confidence);

    // Build pack plan
    let plan = xcprobe_bundle_schema::PackPlan {
        schema_version: "1.0.0".to_string(),
        generated_at: chrono::Utc::now(),
        source_bundle_id: bundle.manifest.collection_id.clone(),
        clusters,
        external_dependencies: vec![],
        startup_dag: dag,
        artifacts: vec![],
        overall_confidence: 0.0,
        warnings: vec![],
    };

    Ok(plan)
}

fn generate_artifacts(
    plan: &xcprobe_bundle_schema::PackPlan,
    output_dir: &PathBuf,
) -> Result<()> {
    for cluster in &plan.clusters {
        let cluster_dir = output_dir.join(&cluster.id);
        std::fs::create_dir_all(&cluster_dir)?;

        // Generate Dockerfile
        let dockerfile = docker::generate_dockerfile(cluster)?;
        std::fs::write(cluster_dir.join("Dockerfile"), dockerfile)?;

        // Generate entrypoint.sh
        let entrypoint = docker::generate_entrypoint(cluster)?;
        std::fs::write(cluster_dir.join("entrypoint.sh"), entrypoint)?;

        // Generate config templates
        for config in &cluster.config_files {
            if config.templated {
                let template = docker::generate_config_template(config)?;
                let template_name = format!(
                    "{}.tmpl",
                    std::path::Path::new(&config.source_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "config".to_string())
                );
                std::fs::write(cluster_dir.join("templates").join(template_name), template)?;
            }
        }

        // Generate README
        let readme = docker::generate_readme(cluster)?;
        std::fs::write(cluster_dir.join("README.md"), readme)?;

        // Generate confidence.json
        let confidence = confidence::generate_confidence_report(cluster)?;
        std::fs::write(cluster_dir.join("confidence.json"), confidence)?;

        info!("Generated artifacts for cluster: {}", cluster.id);
    }

    // Generate docker-compose.yaml
    let compose = docker::generate_compose(plan)?;
    std::fs::write(output_dir.join("docker-compose.yaml"), compose)?;

    Ok(())
}

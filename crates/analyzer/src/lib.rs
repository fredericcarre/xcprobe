//! XCProbe Analyzer - Analyze bundles and generate Docker artifacts.

pub mod clustering;
pub mod confidence;
pub mod dependencies;
pub mod docker;
pub mod scoring;

use anyhow::Result;
use tracing::info;
use xcprobe_bundle_schema::PackPlan;

/// Run the full analysis pipeline on a bundle.
pub fn analyze_bundle(
    bundle: &xcprobe_bundle_schema::Bundle,
    cluster_prefix: &str,
    min_confidence: f64,
) -> Result<PackPlan> {
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
    let plan = PackPlan {
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

/// Generate Docker artifacts from a pack plan.
pub fn generate_artifacts(
    plan: &PackPlan,
    output_dir: &std::path::Path,
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
                let templates_dir = cluster_dir.join("templates");
                std::fs::create_dir_all(&templates_dir)?;
                std::fs::write(templates_dir.join(template_name), template)?;
            }
        }

        // Generate README
        let readme = docker::generate_readme(cluster)?;
        std::fs::write(cluster_dir.join("README.md"), readme)?;

        // Generate confidence.json
        let confidence_report = confidence::generate_confidence_report(cluster)?;
        std::fs::write(cluster_dir.join("confidence.json"), confidence_report)?;

        info!("Generated artifacts for cluster: {}", cluster.id);
    }

    // Generate docker-compose.yaml
    let compose = docker::generate_compose(plan)?;
    std::fs::write(output_dir.join("docker-compose.yaml"), compose)?;

    Ok(())
}

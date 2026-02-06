//! Pack plan generation and execution.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::info;
use xcprobe_bundle_schema::{Bundle, ConfigFileSpec, PackPlan};
use xcprobe_common::OsType;

/// Generate a pack plan from a bundle.
///
/// Note: This is a simplified implementation. The full analyzer crate
/// provides more sophisticated clustering and dependency detection.
pub fn generate_pack_plan(bundle: &Bundle) -> Result<PackPlan> {
    let mut plan = PackPlan::default();
    plan.source_bundle_id = bundle.manifest.collection_id.clone();

    // Collect file paths to pack
    let mut file_paths: Vec<ConfigFileSpec> = Vec::new();

    // Add config files
    for config in &bundle.manifest.config_files {
        file_paths.push(ConfigFileSpec {
            source_path: config.path.clone(),
            container_path: config.path.clone(),
            templated: false,
            template_vars: vec![],
            evidence_ref: config.attachment_ref.clone(),
        });
    }

    // Add environment files
    for env_file in &bundle.manifest.environment_files {
        file_paths.push(ConfigFileSpec {
            source_path: env_file.path.clone(),
            container_path: env_file.path.clone(),
            templated: true,
            template_vars: env_file.variable_names.clone(),
            evidence_ref: env_file.evidence_ref.clone(),
        });
    }

    // Add working directories from services
    for service in &bundle.manifest.services {
        if let Some(ref wd) = service.working_directory {
            file_paths.push(ConfigFileSpec {
                source_path: wd.clone(),
                container_path: wd.clone(),
                templated: false,
                template_vars: vec![],
                evidence_ref: service.evidence_ref.clone(),
            });
        }
    }

    // Store in plan (we'll create a simplified cluster for now)
    plan.overall_confidence = 0.8; // Default confidence

    Ok(plan)
}

/// Execute a pack plan to collect files from target.
pub async fn execute_pack(
    plan: &PackPlan,
    target: &str,
    os_type: OsType,
    ssh_port: u16,
    ssh_user: Option<String>,
    ssh_key: Option<PathBuf>,
    output_dir: &Path,
) -> Result<()> {
    use crate::executor::{Executor, LocalExecutor, SshExecutor};

    std::fs::create_dir_all(output_dir)?;

    // Create executor
    let executor: Box<dyn Executor> = if target == "localhost" || target == "127.0.0.1" {
        Box::new(LocalExecutor::new())
    } else {
        let ssh = SshExecutor::connect(
            target,
            ssh_port,
            ssh_user.as_deref(),
            ssh_key.as_deref(),
            None,
        )?;
        Box::new(ssh)
    };

    // Collect files for each cluster
    for cluster in &plan.clusters {
        let cluster_dir = output_dir.join(&cluster.id);
        std::fs::create_dir_all(&cluster_dir)?;

        for config in &cluster.config_files {
            info!("Collecting: {}", config.source_path);

            let cmd = if os_type.is_linux() {
                format!("cat '{}'", config.source_path)
            } else {
                format!("Get-Content -Path '{}'", config.source_path)
            };

            match executor.execute(&cmd).await {
                Ok((_, content, _)) => {
                    // Determine output path
                    let relative_path = config.source_path.trim_start_matches('/');
                    let output_path = cluster_dir.join(relative_path);

                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    std::fs::write(&output_path, content)?;
                    info!("Wrote: {:?}", output_path);
                }
                Err(e) => {
                    tracing::warn!("Failed to collect {}: {}", config.source_path, e);
                }
            }
        }
    }

    Ok(())
}

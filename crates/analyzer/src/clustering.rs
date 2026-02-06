//! Application clustering logic.

use crate::scoring::ProcessScore;
use anyhow::Result;
use std::collections::HashMap;
use xcprobe_bundle_schema::{
    AppCluster, Bundle, ClusterPort, ClusterProcess, ClusterService, ConfigFileSpec,
    Decision, EnvVarSpec,
};

/// Cluster processes and services into logical applications.
pub fn cluster_applications(
    bundle: &Bundle,
    scores: &HashMap<u32, ProcessScore>,
    prefix: &str,
) -> Result<Vec<AppCluster>> {
    let mut clusters = Vec::new();
    let mut assigned_services: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cluster_id = 0;

    // First, cluster based on services (most reliable grouping)
    for service in &bundle.manifest.services {
        // Skip if already assigned or low-priority system service
        if assigned_services.contains(&service.name) {
            continue;
        }

        // Check if this is a business-relevant service
        let name_lower = service.name.to_lowercase();
        let system_patterns = [
            "systemd-", "dbus", "polkit", "getty", "sshd", "cron",
            "rsyslog", "auditd", "firewalld", "networkmanager",
        ];
        if system_patterns.iter().any(|p| name_lower.contains(p)) {
            continue;
        }

        // Create cluster for this service
        let cluster_name = service
            .name
            .trim_end_matches(".service")
            .replace('.', "-")
            .replace('_', "-");

        let mut cluster = AppCluster {
            id: format!("{}-{}", prefix, cluster_id),
            name: cluster_name.clone(),
            description: service.description.clone(),
            app_type: detect_app_type(service, bundle),
            processes: Vec::new(),
            services: Vec::new(),
            ports: Vec::new(),
            env_vars: Vec::new(),
            config_files: Vec::new(),
            log_paths: Vec::new(),
            depends_on: Vec::new(),
            external_deps: Vec::new(),
            readiness: None,
            confidence: 0.0,
            evidence_refs: Vec::new(),
            decisions: Vec::new(),
        };

        // Add the service
        cluster.services.push(ClusterService {
            name: service.name.clone(),
            exec_start: service.exec_start.clone(),
            user: service.user.clone(),
            working_directory: service.working_directory.clone(),
            environment: service.environment.clone(),
            environment_files: service.environment_files.clone(),
            evidence_ref: service.evidence_ref.clone(),
        });

        // Add evidence reference
        if let Some(ref evidence_ref) = service.evidence_ref {
            cluster.evidence_refs.push(evidence_ref.clone());
        }

        // Add decision about clustering
        cluster.decisions.push(Decision::new(
            format!("Include service {} in cluster", service.name),
            "Service is a business application based on naming and configuration",
            service.evidence_ref.iter().cloned().collect(),
            0.8,
        ));

        // Find associated process
        if let Some(main_pid) = service.main_pid {
            if let Some(proc) = bundle.manifest.processes.iter().find(|p| p.pid == main_pid) {
                cluster.processes.push(ClusterProcess {
                    pid: proc.pid,
                    command: proc.command.clone(),
                    args: proc.args.clone(),
                    user: proc.user.clone(),
                    working_directory: proc.working_directory.clone(),
                    evidence_ref: proc.evidence_ref.clone(),
                });
            }
        }

        // Find associated ports
        if let Some(main_pid) = service.main_pid {
            for port in &bundle.manifest.ports {
                if port.pid == Some(main_pid) {
                    cluster.ports.push(ClusterPort {
                        port: port.local_port,
                        protocol: port.protocol.clone(),
                        purpose: None,
                        evidence_ref: port.evidence_ref.clone(),
                    });

                    cluster.decisions.push(Decision::new(
                        format!("Service listens on port {}", port.local_port),
                        "Port found via ss/netstat associated with service PID",
                        port.evidence_ref.iter().cloned().collect(),
                        0.95,
                    ));
                }
            }
        }

        // Extract environment variables from service
        for (name, _value) in &service.environment {
            let sensitive = xcprobe_redaction::patterns::is_sensitive_key(name);
            cluster.env_vars.push(EnvVarSpec {
                name: name.clone(),
                required: true,
                default_value: if sensitive { None } else { None }, // Don't include values
                description: None,
                sensitive,
                evidence_ref: service.evidence_ref.clone(),
            });
        }

        // Add environment files as configs
        for env_file in &service.environment_files {
            if let Some(file_info) = bundle
                .manifest
                .environment_files
                .iter()
                .find(|f| &f.path == env_file)
            {
                cluster.config_files.push(ConfigFileSpec {
                    source_path: env_file.clone(),
                    container_path: env_file.clone(),
                    templated: true,
                    template_vars: file_info.variable_names.clone(),
                    evidence_ref: file_info.evidence_ref.clone(),
                });

                // Add env var specs for variables found in environment files
                for var_name in &file_info.variable_names {
                    let sensitive = xcprobe_redaction::patterns::is_sensitive_key(var_name);
                    cluster.env_vars.push(EnvVarSpec {
                        name: var_name.clone(),
                        required: true,
                        default_value: None,
                        description: Some(format!("From environment file: {}", env_file)),
                        sensitive,
                        evidence_ref: file_info.evidence_ref.clone(),
                    });
                }
            }
        }

        // Add config files from working directory
        if let Some(ref wd) = service.working_directory {
            for config in &bundle.manifest.config_files {
                if config.path.starts_with(wd) {
                    cluster.config_files.push(ConfigFileSpec {
                        source_path: config.path.clone(),
                        container_path: config.path.clone(),
                        templated: false,
                        template_vars: vec![],
                        evidence_ref: config.attachment_ref.clone(),
                    });
                }
            }
        }

        assigned_services.insert(service.name.clone());
        cluster_id += 1;
        clusters.push(cluster);
    }

    // Second pass: cluster standalone processes with high scores
    for (pid, score) in scores {
        if !score.is_business_process {
            continue;
        }

        // Check if already assigned via service
        let already_assigned = clusters
            .iter()
            .any(|c| c.processes.iter().any(|p| p.pid == *pid));
        if already_assigned {
            continue;
        }

        // Find the process
        let process = match bundle.manifest.processes.iter().find(|p| p.pid == *pid) {
            Some(p) => p,
            None => continue,
        };

        // Create a cluster for this standalone process
        let mut cluster = AppCluster {
            id: format!("{}-{}", prefix, cluster_id),
            name: score.name.clone(),
            description: Some(format!("Standalone process: {}", process.full_cmdline)),
            app_type: "unknown".to_string(),
            processes: vec![ClusterProcess {
                pid: process.pid,
                command: process.command.clone(),
                args: process.args.clone(),
                user: process.user.clone(),
                working_directory: process.working_directory.clone(),
                evidence_ref: process.evidence_ref.clone(),
            }],
            services: Vec::new(),
            ports: Vec::new(),
            env_vars: Vec::new(),
            config_files: Vec::new(),
            log_paths: Vec::new(),
            depends_on: Vec::new(),
            external_deps: Vec::new(),
            readiness: None,
            confidence: 0.0,
            evidence_refs: process.evidence_ref.iter().cloned().collect(),
            decisions: vec![Decision::new(
                format!("Create cluster for process {}", process.command),
                format!("High business relevance score: {:.2}", score.score),
                process.evidence_ref.iter().cloned().collect(),
                score.score,
            )],
        };

        // Find associated ports
        for port in &bundle.manifest.ports {
            if port.pid == Some(*pid) {
                cluster.ports.push(ClusterPort {
                    port: port.local_port,
                    protocol: port.protocol.clone(),
                    purpose: None,
                    evidence_ref: port.evidence_ref.clone(),
                });
            }
        }

        cluster_id += 1;
        clusters.push(cluster);
    }

    Ok(clusters)
}

/// Detect the type of application from service/process characteristics.
fn detect_app_type(service: &xcprobe_bundle_schema::ServiceInfo, bundle: &Bundle) -> String {
    let name_lower = service.name.to_lowercase();
    let exec_lower = service
        .exec_start
        .as_ref()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Check for known application types
    let type_patterns = [
        ("nginx", "proxy"),
        ("apache", "web"),
        ("httpd", "web"),
        ("java", "api"),
        ("node", "api"),
        ("python", "api"),
        ("ruby", "api"),
        ("dotnet", "api"),
        ("postgres", "database"),
        ("mysql", "database"),
        ("mariadb", "database"),
        ("redis", "cache"),
        ("memcached", "cache"),
        ("rabbitmq", "messagequeue"),
        ("kafka", "messagequeue"),
        ("elasticsearch", "search"),
        ("worker", "worker"),
        ("celery", "worker"),
        ("sidekiq", "worker"),
    ];

    for (pattern, app_type) in type_patterns {
        if name_lower.contains(pattern) || exec_lower.contains(pattern) {
            return app_type.to_string();
        }
    }

    // Check ports for hints
    if let Some(main_pid) = service.main_pid {
        for port in &bundle.manifest.ports {
            if port.pid == Some(main_pid) {
                match port.local_port {
                    80 | 443 | 8080 | 8443 => return "web".to_string(),
                    3000 | 5000 | 8000 => return "api".to_string(),
                    5432 => return "database".to_string(),
                    3306 => return "database".to_string(),
                    6379 => return "cache".to_string(),
                    5672 | 15672 => return "messagequeue".to_string(),
                    _ => {}
                }
            }
        }
    }

    "unknown".to_string()
}

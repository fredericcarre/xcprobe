//! System information collector.

use crate::commands::{CommandSet, LinuxCommands, WindowsCommands};
use crate::executor::{Executor, LocalExecutor, SshExecutor, WinRmExecutor};
use crate::parsers;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{debug, info};
use xcprobe_bundle_schema::{AuditEntry, AuditLog, Bundle, Evidence, FileInfo, Manifest};
use xcprobe_common::OsType;
use xcprobe_redaction::Redactor;

/// Collection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMode {
    /// Remote collection via SSH (Linux) or WinRM (Windows).
    Remote,
    /// Local ephemeral collection (for testing).
    LocalEphemeral,
}

impl FromStr for CollectionMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "remote" => Ok(CollectionMode::Remote),
            "local-ephemeral" | "local" => Ok(CollectionMode::LocalEphemeral),
            _ => Err(anyhow::anyhow!("Invalid collection mode: {}", s)),
        }
    }
}

/// Collector configuration.
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub target: String,
    pub os_type: OsType,
    pub mode: CollectionMode,
    pub ssh_port: u16,
    pub ssh_user: Option<String>,
    pub ssh_key: Option<PathBuf>,
    pub ssh_password: Option<String>,
    pub winrm_port: u16,
    pub winrm_user: Option<String>,
    pub winrm_password: Option<String>,
    pub winrm_https: bool,
    #[allow(dead_code)]
    pub timeout_seconds: u64,
}

/// The main collector.
pub struct Collector {
    config: CollectorConfig,
    redactor: Redactor,
}

impl Collector {
    /// Create a new collector.
    pub fn new(config: CollectorConfig) -> Result<Self> {
        Ok(Self {
            config,
            redactor: Redactor::new(),
        })
    }

    /// Run the collection.
    pub async fn collect(&self) -> Result<Bundle> {
        info!(
            "Starting collection for {} ({:?})",
            self.config.target, self.config.os_type
        );

        let mut manifest = Manifest {
            collection_mode: format!("{:?}", self.config.mode).to_lowercase(),
            ..Default::default()
        };

        let mut audit_log = AuditLog::new();
        let mut evidence: HashMap<String, Evidence> = HashMap::new();
        let mut checksums: HashMap<String, String> = HashMap::new();

        // Create executor
        let executor = self.create_executor().await?;

        // Get command set based on OS
        let commands: Box<dyn CommandSet> = match self.config.os_type {
            OsType::Linux => Box::new(LinuxCommands::new()),
            OsType::Windows => Box::new(WindowsCommands::new()),
        };

        // Collect system info
        info!("Collecting system information...");
        self.collect_system_info(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect processes
        info!("Collecting process information...");
        self.collect_processes(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect services
        info!("Collecting service information...");
        self.collect_services(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect ports
        info!("Collecting port information...");
        self.collect_ports(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect packages
        info!("Collecting package information...");
        self.collect_packages(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect scheduled tasks
        info!("Collecting scheduled tasks...");
        self.collect_scheduled_tasks(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect config files based on discovered services
        info!("Collecting configuration files...");
        self.collect_config_files(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        // Collect log snippets
        info!("Collecting log snippets...");
        self.collect_logs(
            &*executor,
            commands.as_ref(),
            &mut manifest,
            &mut audit_log,
            &mut evidence,
        )
        .await?;

        manifest.completed_at = Some(Utc::now());

        // Compute checksums for all evidence
        for (path, ev) in &evidence {
            checksums.insert(path.clone(), ev.content_hash.clone());
        }

        Ok(Bundle {
            manifest,
            audit: audit_log.entries().to_vec(),
            evidence,
            checksums,
        })
    }

    async fn create_executor(&self) -> Result<Box<dyn Executor>> {
        match self.config.mode {
            CollectionMode::LocalEphemeral => Ok(Box::new(LocalExecutor::new())),
            CollectionMode::Remote => match self.config.os_type {
                OsType::Linux => {
                    let executor = SshExecutor::connect(
                        &self.config.target,
                        self.config.ssh_port,
                        self.config.ssh_user.as_deref(),
                        self.config.ssh_key.as_deref(),
                        self.config.ssh_password.as_deref(),
                    )?;
                    Ok(Box::new(executor))
                }
                OsType::Windows => {
                    let executor = WinRmExecutor::connect(
                        &self.config.target,
                        self.config.winrm_port,
                        self.config.winrm_https,
                        self.config.winrm_user.as_deref(),
                        self.config.winrm_password.as_deref(),
                    )
                    .await?;
                    Ok(Box::new(executor))
                }
            },
        }
    }

    async fn collect_system_info(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        let cmd = commands.hostname_cmd();
        let result = self
            .execute_and_record(executor, cmd, "system", audit_log, evidence)
            .await?;
        manifest.system.hostname = result.stdout.trim().to_string();
        manifest.system.os_type = self.config.os_type.to_string();

        if let Some(cmd) = commands.os_version_cmd() {
            if let Ok(result) = self
                .execute_and_record(executor, cmd, "system", audit_log, evidence)
                .await
            {
                manifest.system.os_version = Some(result.stdout.trim().to_string());
            }
        }

        if let Some(cmd) = commands.kernel_version_cmd() {
            if let Ok(result) = self
                .execute_and_record(executor, cmd, "system", audit_log, evidence)
                .await
            {
                manifest.system.kernel_version = Some(result.stdout.trim().to_string());
            }
        }

        if let Some(cmd) = commands.architecture_cmd() {
            if let Ok(result) = self
                .execute_and_record(executor, cmd, "system", audit_log, evidence)
                .await
            {
                manifest.system.architecture = Some(result.stdout.trim().to_string());
            }
        }

        Ok(())
    }

    async fn collect_processes(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        for cmd in commands.process_cmds() {
            let result = self
                .execute_and_record(executor, cmd, "process", audit_log, evidence)
                .await;
            if let Ok(result) = result {
                let processes = parsers::parse_processes(&result.stdout, self.config.os_type)?;
                for mut proc in processes {
                    proc.evidence_ref = Some(result.evidence_ref.clone());
                    manifest.processes.push(proc);
                }
            }
        }
        Ok(())
    }

    async fn collect_services(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        let list_cmd = commands.service_list_cmd();
        let result = self
            .execute_and_record(executor, list_cmd, "service", audit_log, evidence)
            .await?;

        if self.config.os_type.is_windows() {
            // Windows: parse full details directly from the list output (single query)
            let mut services =
                parsers::parse_windows_services_from_list(&result.stdout)?;
            for service in &mut services {
                service.evidence_ref = Some(result.evidence_ref.clone());
            }
            manifest.services.extend(services);
        } else {
            // Linux: list names then query each service for details + unit files
            let service_names =
                parsers::parse_service_list(&result.stdout, self.config.os_type)?;

            for name in service_names {
                if let Some(show_cmd) = commands.service_show_cmd(&name) {
                    if let Ok(show_result) = self
                        .execute_and_record(executor, &show_cmd, "service", audit_log, evidence)
                        .await
                    {
                        if let Ok(mut service) = parsers::parse_service_details(
                            &show_result.stdout,
                            self.config.os_type,
                        ) {
                            service.evidence_ref = Some(show_result.evidence_ref.clone());

                            if let Some(cat_cmd) = commands.service_cat_cmd(&name) {
                                if let Ok(cat_result) = self
                                    .execute_and_record(
                                        executor, &cat_cmd, "service", audit_log, evidence,
                                    )
                                    .await
                                {
                                    let unit_info =
                                        parsers::parse_systemd_unit(&cat_result.stdout);
                                    if let Some(exec) = unit_info.exec_start {
                                        service.exec_start = Some(exec);
                                    }
                                    if let Some(wd) = unit_info.working_directory {
                                        service.working_directory = Some(wd);
                                    }
                                    service
                                        .environment_files
                                        .extend(unit_info.environment_files);
                                }
                            }

                            manifest.services.push(service);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn collect_ports(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        let cmd = commands.ports_cmd();
        let result = self
            .execute_and_record(executor, cmd, "ports", audit_log, evidence)
            .await?;
        let ports = parsers::parse_ports(&result.stdout, self.config.os_type)?;

        for mut port in ports {
            port.evidence_ref = Some(result.evidence_ref.clone());
            manifest.ports.push(port);
        }

        Ok(())
    }

    async fn collect_packages(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        for cmd in commands.package_cmds() {
            if let Ok(result) = self
                .execute_and_record(executor, cmd, "packages", audit_log, evidence)
                .await
            {
                let packages = parsers::parse_packages(&result.stdout, self.config.os_type, cmd)?;
                manifest.packages.extend(packages);
                break; // Only use first successful package manager
            }
        }
        Ok(())
    }

    async fn collect_scheduled_tasks(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        for cmd in commands.scheduled_task_cmds() {
            if let Ok(result) = self
                .execute_and_record(executor, cmd, "scheduled_tasks", audit_log, evidence)
                .await
            {
                let tasks = parsers::parse_scheduled_tasks(&result.stdout, self.config.os_type)?;
                for mut task in tasks {
                    task.evidence_ref = Some(result.evidence_ref.clone());
                    manifest.scheduled_tasks.push(task);
                }
            }
        }
        Ok(())
    }

    async fn collect_config_files(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        // Collect config files from known service paths
        let mut config_paths: Vec<String> = Vec::new();

        // Add paths from services
        for service in &manifest.services {
            if let Some(ref wd) = service.working_directory {
                config_paths.push(wd.clone());
            }
            for env_file in &service.environment_files {
                config_paths.push(env_file.clone());
            }
        }

        // Add paths from process working directories
        for process in &manifest.processes {
            if let Some(ref wd) = process.working_directory {
                config_paths.push(wd.clone());
            }
        }

        // Standard config paths
        let standard_paths = if self.config.os_type.is_linux() {
            vec!["/etc/default", "/etc/sysconfig"]
        } else {
            vec!["C:\\ProgramData"]
        };

        for path in standard_paths {
            config_paths.push(path.to_string());
        }

        // Deduplicate and collect
        config_paths.sort();
        config_paths.dedup();

        for path in &config_paths {
            if let Some(cmd) = commands.read_file_cmd(path) {
                if let Ok(result) = self
                    .execute_and_record(executor, &cmd, "config", audit_log, evidence)
                    .await
                {
                    // Redact content before storing
                    let redacted = self.redactor.redact(&result.stdout);
                    let file_info = FileInfo {
                        path: path.clone(),
                        size_bytes: result.stdout.len() as u64,
                        modified_at: None,
                        owner: None,
                        permissions: None,
                        content_hash: Some(xcprobe_common::hash::sha256_str(&redacted.content)),
                        attachment_ref: Some(result.evidence_ref.clone()),
                        discovery_method: "service_path".to_string(),
                        discovery_evidence_ref: None,
                    };
                    manifest.config_files.push(file_info);
                }
            }
        }

        Ok(())
    }

    async fn collect_logs(
        &self,
        executor: &dyn Executor,
        commands: &dyn CommandSet,
        manifest: &mut Manifest,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<()> {
        // Collect journal logs for each service (Linux)
        if self.config.os_type.is_linux() {
            for service in &manifest.services {
                if let Some(cmd) = commands.journal_cmd(&service.name, "1 hour ago") {
                    if let Ok(result) = self
                        .execute_and_record(executor, &cmd, "logs", audit_log, evidence)
                        .await
                    {
                        let redacted = self.redactor.redact(&result.stdout);
                        let file_info = FileInfo {
                            path: format!("journal:{}", service.name),
                            size_bytes: result.stdout.len() as u64,
                            modified_at: None,
                            owner: None,
                            permissions: None,
                            content_hash: Some(xcprobe_common::hash::sha256_str(&redacted.content)),
                            attachment_ref: Some(result.evidence_ref.clone()),
                            discovery_method: "journalctl".to_string(),
                            discovery_evidence_ref: None,
                        };
                        manifest.log_files.push(file_info);
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_and_record(
        &self,
        executor: &dyn Executor,
        command: &str,
        category: &str,
        audit_log: &mut AuditLog,
        evidence: &mut HashMap<String, Evidence>,
    ) -> Result<ExecutionResult> {
        let started_at = Utc::now();
        debug!("Executing: {}", command);

        let (exit_code, stdout, stderr) = executor.execute(command).await?;

        let completed_at = Utc::now();
        let evidence_id = format!("{}_{}", category, uuid::Uuid::new_v4());
        let evidence_ref = format!("evidence/{}.txt", evidence_id);

        // Create evidence
        let content = format!("=== STDOUT ===\n{}\n\n=== STDERR ===\n{}", stdout, stderr);
        let redacted = self.redactor.redact(&content);
        let ev = Evidence::from_command_output(
            &evidence_id,
            command,
            redacted.content.into_bytes(),
            &evidence_ref,
        );
        evidence.insert(evidence_ref.clone(), ev);

        // Create audit entry
        let audit_entry = AuditEntry::new(
            0, // Will be set by AuditLog
            command.to_string(),
            category.to_string(),
            started_at,
            completed_at,
            exit_code,
            stdout.len() as u64,
            stderr.len() as u64,
            evidence_ref.clone(),
            if exit_code.map(|c| c != 0).unwrap_or(true) {
                Some(stderr.clone())
            } else {
                None
            },
        );
        audit_log.add(audit_entry);

        Ok(ExecutionResult {
            exit_code,
            stdout,
            stderr,
            evidence_ref,
        })
    }
}

struct ExecutionResult {
    #[allow(dead_code)]
    exit_code: Option<i32>,
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
    evidence_ref: String,
}

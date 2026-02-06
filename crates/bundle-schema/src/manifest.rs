//! Manifest types for the collection bundle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete bundle structure (in-memory representation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub manifest: Manifest,
    pub audit: Vec<super::AuditEntry>,
    pub evidence: HashMap<String, super::Evidence>,
    pub checksums: HashMap<String, String>,
}

/// The manifest.json file - contains all collected facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version for forward compatibility.
    pub schema_version: String,
    /// Unique identifier for this collection.
    pub collection_id: String,
    /// When the collection started.
    pub collected_at: DateTime<Utc>,
    /// When the collection completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Target system information.
    pub system: SystemInfo,
    /// Collected processes.
    pub processes: Vec<ProcessInfo>,
    /// Collected services (systemd units or Windows services).
    pub services: Vec<ServiceInfo>,
    /// Open ports and listeners.
    pub ports: Vec<PortInfo>,
    /// Network connections.
    pub connections: Vec<NetworkConnection>,
    /// Installed packages.
    pub packages: Vec<Package>,
    /// Scheduled tasks (cron, systemd timers, Windows scheduled tasks).
    pub scheduled_tasks: Vec<ScheduledTask>,
    /// Collected configuration files.
    pub config_files: Vec<FileInfo>,
    /// Collected log snippets.
    pub log_files: Vec<FileInfo>,
    /// Environment files found.
    pub environment_files: Vec<EnvironmentFile>,
    /// Collection mode used.
    pub collection_mode: String,
    /// Any errors encountered during collection.
    pub errors: Vec<CollectionError>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: "1.0.0".to_string(),
            collection_id: uuid::Uuid::new_v4().to_string(),
            collected_at: Utc::now(),
            completed_at: None,
            system: SystemInfo::default(),
            processes: Vec::new(),
            services: Vec::new(),
            ports: Vec::new(),
            connections: Vec::new(),
            packages: Vec::new(),
            scheduled_tasks: Vec::new(),
            config_files: Vec::new(),
            log_files: Vec::new(),
            environment_files: Vec::new(),
            collection_mode: "unknown".to_string(),
            errors: Vec::new(),
        }
    }
}

/// System information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_type: String,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub timezone: Option<String>,
}

/// Process information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    pub command: String,
    pub args: Vec<String>,
    pub full_cmdline: String,
    pub start_time: Option<DateTime<Utc>>,
    pub elapsed_time: Option<String>,
    pub cpu_percent: Option<f32>,
    pub memory_percent: Option<f32>,
    pub working_directory: Option<String>,
    pub environment: Option<HashMap<String, String>>,
    /// Evidence reference for the raw ps output.
    pub evidence_ref: Option<String>,
}

/// Service information (systemd or Windows service).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub state: String,
    pub sub_state: Option<String>,
    pub start_mode: Option<String>,
    pub exec_start: Option<String>,
    pub exec_start_pre: Vec<String>,
    pub exec_start_post: Vec<String>,
    pub exec_stop: Option<String>,
    pub working_directory: Option<String>,
    pub user: Option<String>,
    pub group: Option<String>,
    pub environment: HashMap<String, String>,
    pub environment_files: Vec<String>,
    pub unit_file_path: Option<String>,
    pub dependencies: Vec<String>,
    pub wanted_by: Vec<String>,
    pub main_pid: Option<u32>,
    /// Evidence reference for the unit file content.
    pub evidence_ref: Option<String>,
}

/// Port/listener information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub protocol: String,
    pub local_address: String,
    pub local_port: u16,
    pub state: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    /// Evidence reference.
    pub evidence_ref: Option<String>,
}

/// Network connection information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub protocol: String,
    pub local_address: String,
    pub local_port: u16,
    pub remote_address: String,
    pub remote_port: u16,
    pub state: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

/// Package information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub architecture: Option<String>,
    pub description: Option<String>,
    pub install_date: Option<DateTime<Utc>>,
    pub source: String, // dpkg, rpm, etc.
}

/// Scheduled task information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub name: String,
    pub task_type: String, // cron, systemd-timer, windows-task
    pub schedule: Option<String>,
    pub command: Option<String>,
    pub user: Option<String>,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    /// Evidence reference.
    pub evidence_ref: Option<String>,
}

/// File information for configs and logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub owner: Option<String>,
    pub permissions: Option<String>,
    pub content_hash: Option<String>,
    /// Reference to the attachment containing (redacted) content.
    pub attachment_ref: Option<String>,
    /// How this file was discovered.
    pub discovery_method: String,
    /// Evidence that led to discovering this file.
    pub discovery_evidence_ref: Option<String>,
}

/// Environment file information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFile {
    pub path: String,
    /// Variable names found (not values - those may be sensitive).
    pub variable_names: Vec<String>,
    /// Evidence reference.
    pub evidence_ref: Option<String>,
}

/// Collection error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionError {
    pub phase: String,
    pub command: Option<String>,
    pub error: String,
    pub timestamp: DateTime<Utc>,
    pub recoverable: bool,
}

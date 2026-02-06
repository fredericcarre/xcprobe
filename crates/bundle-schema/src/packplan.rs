//! Pack plan types - output of the analyzer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The complete pack plan - describes how to containerize the discovered applications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackPlan {
    /// Schema version.
    pub schema_version: String,
    /// When this plan was generated.
    pub generated_at: DateTime<Utc>,
    /// Source bundle ID.
    pub source_bundle_id: String,
    /// Discovered application clusters.
    pub clusters: Vec<AppCluster>,
    /// Global dependencies (external endpoints).
    pub external_dependencies: Vec<DependencyInfo>,
    /// Startup order DAG (edges from dependency to dependent).
    pub startup_dag: Vec<DagEdge>,
    /// Generated artifacts metadata.
    pub artifacts: Vec<GeneratedArtifact>,
    /// Overall confidence score (0.0 - 1.0).
    pub overall_confidence: f64,
    /// Analysis warnings.
    pub warnings: Vec<AnalysisWarning>,
}

impl Default for PackPlan {
    fn default() -> Self {
        Self {
            schema_version: "1.0.0".to_string(),
            generated_at: chrono::Utc::now(),
            source_bundle_id: String::new(),
            clusters: Vec::new(),
            external_dependencies: Vec::new(),
            startup_dag: Vec::new(),
            artifacts: Vec::new(),
            overall_confidence: 0.0,
            warnings: Vec::new(),
        }
    }
}

/// An application cluster - a logical grouping of related processes/services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCluster {
    /// Unique identifier for this cluster.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the application.
    pub description: Option<String>,
    /// Type of application (web, api, worker, database, cache, batch, etc.).
    pub app_type: String,
    /// Component processes.
    pub processes: Vec<ClusterProcess>,
    /// Component services.
    pub services: Vec<ClusterService>,
    /// Exposed ports.
    pub ports: Vec<ClusterPort>,
    /// Required environment variables (names only).
    pub env_vars: Vec<EnvVarSpec>,
    /// Configuration files.
    pub config_files: Vec<ConfigFileSpec>,
    /// Log files/paths.
    pub log_paths: Vec<String>,
    /// Dependencies on other clusters.
    pub depends_on: Vec<String>,
    /// Dependencies on external endpoints.
    pub external_deps: Vec<String>,
    /// Readiness check configuration.
    pub readiness: Option<ReadinessCheck>,
    /// Confidence score for this cluster (0.0 - 1.0).
    pub confidence: f64,
    /// Evidence references that support this cluster identification.
    pub evidence_refs: Vec<String>,
    /// Decisions made and their justifications.
    pub decisions: Vec<Decision>,
}

/// A process within a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterProcess {
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
    pub user: String,
    pub working_directory: Option<String>,
    pub evidence_ref: Option<String>,
}

/// A service within a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterService {
    pub name: String,
    pub exec_start: Option<String>,
    pub user: Option<String>,
    pub working_directory: Option<String>,
    pub environment: HashMap<String, String>,
    pub environment_files: Vec<String>,
    pub evidence_ref: Option<String>,
}

/// Port exposed by a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterPort {
    pub port: u16,
    pub protocol: String,
    pub purpose: Option<String>,
    pub evidence_ref: Option<String>,
}

/// Environment variable specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSpec {
    /// Variable name.
    pub name: String,
    /// Whether this variable is required.
    pub required: bool,
    /// Default value (if not sensitive).
    pub default_value: Option<String>,
    /// Description of what this variable is for.
    pub description: Option<String>,
    /// Whether the value is likely sensitive.
    pub sensitive: bool,
    /// Evidence reference.
    pub evidence_ref: Option<String>,
}

/// Configuration file specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileSpec {
    /// Original path on the source system.
    pub source_path: String,
    /// Suggested path in the container.
    pub container_path: String,
    /// Whether this file uses templating.
    pub templated: bool,
    /// Template variables used.
    pub template_vars: Vec<String>,
    /// Evidence reference.
    pub evidence_ref: Option<String>,
}

/// Readiness check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheck {
    /// Type of check (tcp, http, command).
    pub check_type: String,
    /// Target for TCP/HTTP checks.
    pub target: Option<String>,
    /// Port for TCP checks.
    pub port: Option<u16>,
    /// Path for HTTP checks.
    pub path: Option<String>,
    /// Command for command-based checks.
    pub command: Option<String>,
    /// Timeout in seconds.
    pub timeout_seconds: u32,
    /// Interval between checks.
    pub interval_seconds: u32,
    /// Number of retries.
    pub retries: u32,
}

/// Dependency on an external endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Unique identifier.
    pub id: String,
    /// Type (database, api, cache, messagequeue, etc.).
    pub dep_type: String,
    /// Endpoint found in config.
    pub endpoint: String,
    /// Port if detected.
    pub port: Option<u16>,
    /// Which clusters depend on this.
    pub used_by: Vec<String>,
    /// Evidence references.
    pub evidence_refs: Vec<String>,
}

/// DAG edge for startup order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdge {
    /// Cluster or dependency that must start first.
    pub from: String,
    /// Cluster that depends on `from`.
    pub to: String,
    /// Reason for this dependency.
    pub reason: String,
}

/// Generated artifact metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedArtifact {
    /// Cluster ID this artifact belongs to.
    pub cluster_id: String,
    /// Type of artifact.
    pub artifact_type: ArtifactType,
    /// Path where the artifact was written.
    pub path: String,
    /// Description.
    pub description: Option<String>,
}

/// Type of generated artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Dockerfile,
    Entrypoint,
    ConfigTemplate,
    ComposeFile,
    Readme,
    ConfidenceReport,
}

/// A decision made during analysis with justification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// What decision was made.
    pub decision: String,
    /// Reason for the decision.
    pub reason: String,
    /// Evidence references supporting this decision.
    pub evidence_refs: Vec<String>,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f64,
}

impl Decision {
    /// Create a new decision with evidence.
    pub fn new(
        decision: impl Into<String>,
        reason: impl Into<String>,
        evidence_refs: Vec<String>,
        confidence: f64,
    ) -> Self {
        Self {
            decision: decision.into(),
            reason: reason.into(),
            evidence_refs,
            confidence,
        }
    }

    /// Check if this decision has sufficient evidence.
    pub fn has_evidence(&self) -> bool {
        !self.evidence_refs.is_empty()
    }
}

/// Analysis warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisWarning {
    /// Warning code.
    pub code: String,
    /// Warning message.
    pub message: String,
    /// Severity (info, warning, error).
    pub severity: String,
    /// Affected cluster IDs.
    pub affected_clusters: Vec<String>,
}

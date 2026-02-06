//! Bundle schema definitions for xcprobe.
//!
//! This crate defines the structure of collection bundles,
//! including manifest, audit logs, and evidence.

pub mod audit;
pub mod evidence;
pub mod manifest;
pub mod packplan;
pub mod schema;
pub mod validation;

pub use audit::{AuditEntry, AuditLog};
pub use evidence::{Evidence, EvidenceRef, EvidenceType};
pub use manifest::{
    Bundle, EnvironmentFile, FileInfo, Manifest, NetworkConnection, Package, PortInfo, ProcessInfo,
    ScheduledTask, ServiceInfo, SystemInfo,
};
pub use packplan::{
    AppCluster, ConfigFileSpec, DagEdge, Decision, DependencyInfo, GeneratedArtifact, PackPlan,
    ReadinessCheck,
};
pub use validation::validate_bundle;

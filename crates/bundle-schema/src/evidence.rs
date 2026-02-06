//! Evidence types for storing raw command outputs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// Raw command output (stdout + stderr).
    CommandOutput,
    /// Configuration file content.
    ConfigFile,
    /// Log file snippet.
    LogSnippet,
    /// Environment file content.
    EnvFile,
    /// Unit file content.
    UnitFile,
    /// Other file content.
    FileContent,
}

/// A reference to evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Path within the bundle (e.g., "evidence/ps_auxww.txt").
    pub path: String,
    /// Type of evidence.
    pub evidence_type: EvidenceType,
    /// Description of what this evidence contains.
    pub description: String,
}

impl EvidenceRef {
    pub fn new(path: impl Into<String>, evidence_type: EvidenceType, description: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            evidence_type,
            description: description.into(),
        }
    }
}

/// Evidence metadata and content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Unique identifier for this evidence.
    pub id: String,
    /// Type of evidence.
    pub evidence_type: EvidenceType,
    /// When this evidence was collected.
    pub collected_at: DateTime<Utc>,
    /// The command that produced this evidence (if applicable).
    pub source_command: Option<String>,
    /// Size in bytes.
    pub size_bytes: u64,
    /// SHA256 hash of the content.
    pub content_hash: String,
    /// Whether this content has been redacted.
    pub redacted: bool,
    /// Path within the bundle.
    pub bundle_path: String,
    /// Original path on the target system (if applicable).
    pub original_path: Option<String>,
    /// The actual content (only present in memory, not serialized to metadata).
    #[serde(skip)]
    pub content: Option<Vec<u8>>,
}

impl Evidence {
    /// Create evidence from command output.
    pub fn from_command_output(
        id: impl Into<String>,
        command: impl Into<String>,
        content: Vec<u8>,
        bundle_path: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let content_hash = xcprobe_common::hash::sha256_bytes(&content);
        let size_bytes = content.len() as u64;

        Self {
            id,
            evidence_type: EvidenceType::CommandOutput,
            collected_at: chrono::Utc::now(),
            source_command: Some(command.into()),
            size_bytes,
            content_hash,
            redacted: false,
            bundle_path: bundle_path.into(),
            original_path: None,
            content: Some(content),
        }
    }

    /// Create evidence from a file.
    pub fn from_file(
        id: impl Into<String>,
        evidence_type: EvidenceType,
        content: Vec<u8>,
        bundle_path: impl Into<String>,
        original_path: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let content_hash = xcprobe_common::hash::sha256_bytes(&content);
        let size_bytes = content.len() as u64;

        Self {
            id,
            evidence_type,
            collected_at: chrono::Utc::now(),
            source_command: None,
            size_bytes,
            content_hash,
            redacted: false,
            bundle_path: bundle_path.into(),
            original_path: Some(original_path.into()),
            content: Some(content),
        }
    }

    /// Mark this evidence as redacted.
    pub fn mark_redacted(&mut self) {
        self.redacted = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_from_command() {
        let evidence = Evidence::from_command_output(
            "ps_001",
            "ps auxww",
            b"USER PID...".to_vec(),
            "evidence/ps_auxww.txt",
        );
        assert_eq!(evidence.evidence_type, EvidenceType::CommandOutput);
        assert!(!evidence.redacted);
    }
}

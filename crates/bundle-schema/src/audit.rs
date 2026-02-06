//! Audit log types for tracking all commands executed during collection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique sequence number.
    pub seq: u64,
    /// When the command started.
    pub started_at: DateTime<Utc>,
    /// When the command completed.
    pub completed_at: DateTime<Utc>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// The command that was executed.
    pub command: String,
    /// Exit code of the command.
    pub exit_code: Option<i32>,
    /// Whether the command succeeded.
    pub success: bool,
    /// Size of stdout in bytes.
    pub stdout_bytes: u64,
    /// Size of stderr in bytes.
    pub stderr_bytes: u64,
    /// Reference to the evidence file containing the output.
    pub evidence_ref: String,
    /// Any error message if the command failed.
    pub error: Option<String>,
    /// Category of the command (process, service, port, etc.).
    pub category: String,
}

impl AuditEntry {
    /// Create a new audit entry.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seq: u64,
        command: String,
        category: String,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        exit_code: Option<i32>,
        stdout_bytes: u64,
        stderr_bytes: u64,
        evidence_ref: String,
        error: Option<String>,
    ) -> Self {
        let duration = completed_at - started_at;
        let duration_ms = duration.num_milliseconds().max(0) as u64;
        let success = exit_code.map(|c| c == 0).unwrap_or(false);

        Self {
            seq,
            started_at,
            completed_at,
            duration_ms,
            command,
            exit_code,
            success,
            stdout_bytes,
            stderr_bytes,
            evidence_ref,
            error,
            category,
        }
    }
}

/// Collection of audit entries (written as JSONL).
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    next_seq: u64,
}

impl AuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry to the audit log.
    pub fn add(&mut self, mut entry: AuditEntry) {
        entry.seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push(entry);
    }

    /// Get all entries.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Convert to JSONL format.
    pub fn to_jsonl(&self) -> String {
        self.entries
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Parse from JSONL format.
    pub fn from_jsonl(content: &str) -> Result<Self, serde_json::Error> {
        let mut log = Self::new();
        for line in content.lines() {
            if !line.trim().is_empty() {
                let entry: AuditEntry = serde_json::from_str(line)?;
                log.entries.push(entry);
            }
        }
        if let Some(last) = log.entries.last() {
            log.next_seq = last.seq + 1;
        }
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_jsonl() {
        let mut log = AuditLog::new();
        let now = Utc::now();
        log.add(AuditEntry::new(
            0,
            "ps auxww".to_string(),
            "process".to_string(),
            now,
            now + chrono::Duration::milliseconds(100),
            Some(0),
            1024,
            0,
            "evidence/ps_auxww.txt".to_string(),
            None,
        ));

        let jsonl = log.to_jsonl();
        let parsed = AuditLog::from_jsonl(&jsonl).unwrap();
        assert_eq!(parsed.entries().len(), 1);
    }
}

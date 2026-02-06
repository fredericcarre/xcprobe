//! Common error types for xcprobe.

use thiserror::Error;

/// Common error type for xcprobe operations.
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SSH connection failed: {0}")]
    SshConnection(String),

    #[error("SSH authentication failed: {0}")]
    SshAuth(String),

    #[error("Command execution failed: {cmd} - {reason}")]
    CommandExecution { cmd: String, reason: String },

    #[error("Command timed out: {cmd}")]
    CommandTimeout { cmd: String },

    #[error("Invalid bundle: {0}")]
    InvalidBundle(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),

    #[error("Missing evidence for decision: {decision}")]
    MissingEvidence { decision: String },

    #[error("Unsupported OS: {0}")]
    UnsupportedOs(String),

    #[error("WinRM connection failed: {0}")]
    WinRmConnection(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Redaction error: {0}")]
    Redaction(String),

    #[error("Analysis error: {0}")]
    Analysis(String),

    #[error("Pack error: {0}")]
    Pack(String),

    #[error("{0}")]
    Other(String),
}

/// Result type alias using common Error.
pub type Result<T> = std::result::Result<T, Error>;

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Other(e.to_string())
    }
}

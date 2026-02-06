//! Truth file handling.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Ground truth for a test scenario.
#[derive(Debug, Serialize, Deserialize)]
pub struct Truth {
    /// Schema version
    pub version: String,
    /// Scenario name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Expected applications
    pub applications: Vec<ExpectedApp>,
    /// Expected ports
    pub ports: Vec<ExpectedPort>,
    /// Expected environment variable names
    pub env_names: Vec<String>,
    /// Expected dependencies
    pub dependencies: Vec<ExpectedDependency>,
    /// Expected config files
    pub config_files: Vec<String>,
    /// Thresholds for pass/fail
    #[serde(default)]
    pub thresholds: Thresholds,
}

/// Expected application.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpectedApp {
    /// Application name
    pub name: String,
    /// Application type
    pub app_type: String,
    /// Expected command patterns
    pub command_patterns: Vec<String>,
    /// Expected user
    pub user: Option<String>,
    /// Expected working directory
    pub working_directory: Option<String>,
}

/// Expected port.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpectedPort {
    pub port: u16,
    pub protocol: String,
    pub process_name: Option<String>,
}

/// Expected dependency.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpectedDependency {
    pub from: String,
    pub to: String,
    pub dep_type: String,
}

/// Thresholds for pass/fail determination.
#[derive(Debug, Serialize, Deserialize)]
pub struct Thresholds {
    /// Minimum process/cmdline recall
    #[serde(default = "default_process_recall")]
    pub process_cmdline_recall: f64,
    /// Minimum ports recall
    #[serde(default = "default_ports_recall")]
    pub ports_recall: f64,
    /// Minimum env names recall
    #[serde(default = "default_env_recall")]
    pub env_names_recall: f64,
    /// Minimum dependencies recall
    #[serde(default = "default_deps_recall")]
    pub deps_recall: f64,
    /// All decisions must have evidence
    #[serde(default = "default_require_evidence")]
    pub require_all_evidence: bool,
}

fn default_process_recall() -> f64 {
    0.9
}
fn default_ports_recall() -> f64 {
    0.95
}
fn default_env_recall() -> f64 {
    0.8
}
fn default_deps_recall() -> f64 {
    0.8
}
fn default_require_evidence() -> bool {
    true
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            process_cmdline_recall: default_process_recall(),
            ports_recall: default_ports_recall(),
            env_names_recall: default_env_recall(),
            deps_recall: default_deps_recall(),
            require_all_evidence: default_require_evidence(),
        }
    }
}

/// Load truth from file.
pub fn load_truth(path: &Path) -> Result<Truth> {
    let content = std::fs::read_to_string(path).context("Failed to read truth file")?;
    let truth: Truth = serde_json::from_str(&content).context("Failed to parse truth file")?;
    Ok(truth)
}

/// Validation result.
#[derive(Debug)]
pub struct TruthValidation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate truth file structure.
pub fn validate_truth(truth: &Truth) -> TruthValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check required fields
    if truth.name.is_empty() {
        errors.push("Missing scenario name".to_string());
    }

    if truth.applications.is_empty() {
        warnings.push("No expected applications defined".to_string());
    }

    // Check thresholds
    if truth.thresholds.process_cmdline_recall > 1.0 {
        errors.push("process_cmdline_recall threshold cannot exceed 1.0".to_string());
    }

    if truth.thresholds.ports_recall > 1.0 {
        errors.push("ports_recall threshold cannot exceed 1.0".to_string());
    }

    // Check for duplicate ports
    let mut seen_ports = std::collections::HashSet::new();
    for port in &truth.ports {
        if !seen_ports.insert(port.port) {
            warnings.push(format!("Duplicate port: {}", port.port));
        }
    }

    TruthValidation { errors, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_truth() {
        let json = r#"{
            "version": "1.0",
            "name": "test-scenario",
            "applications": [
                {
                    "name": "api",
                    "app_type": "api",
                    "command_patterns": ["python", "flask"]
                }
            ],
            "ports": [
                {"port": 8080, "protocol": "tcp"}
            ],
            "env_names": ["DATABASE_URL"],
            "dependencies": [],
            "config_files": ["/etc/app/config.yaml"]
        }"#;

        let truth: Truth = serde_json::from_str(json).unwrap();
        assert_eq!(truth.name, "test-scenario");
        assert_eq!(truth.applications.len(), 1);
        assert_eq!(truth.ports[0].port, 8080);
    }

    #[test]
    fn test_validate_truth() {
        let truth = Truth {
            version: "1.0".to_string(),
            name: "".to_string(), // Invalid: empty name
            description: None,
            applications: vec![],
            ports: vec![],
            env_names: vec![],
            dependencies: vec![],
            config_files: vec![],
            thresholds: Thresholds::default(),
        };

        let result = validate_truth(&truth);
        assert!(!result.errors.is_empty());
    }
}

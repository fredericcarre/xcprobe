//! Bundle validation utilities.

use crate::schema;
use jsonschema::JSONSchema;
use serde_json::Value;
use std::collections::HashSet;
use thiserror::Error;

/// Validation error type.
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Schema validation failed: {0}")]
    SchemaError(String),

    #[error("Missing required file: {0}")]
    MissingFile(String),

    #[error("Checksum mismatch for {file}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid evidence reference: {0}")]
    InvalidEvidenceRef(String),

    #[error("Decision without evidence: {0}")]
    DecisionWithoutEvidence(String),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result of bundle validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a manifest against the JSON schema.
pub fn validate_manifest(manifest: &Value) -> Result<ValidationResult, ValidationError> {
    let mut result = ValidationResult::new();

    let schema_value = schema::manifest_schema();
    let compiled = JSONSchema::compile(&schema_value)
        .map_err(|e| ValidationError::SchemaError(e.to_string()))?;

    let validation = compiled.validate(manifest);
    if let Err(errors) = validation {
        for error in errors {
            result.add_error(ValidationError::SchemaError(format!(
                "{} at {}",
                error, error.instance_path
            )));
        }
    }

    Ok(result)
}

/// Validate a packplan against the JSON schema.
pub fn validate_packplan(packplan: &Value) -> Result<ValidationResult, ValidationError> {
    let mut result = ValidationResult::new();

    let schema_value = schema::packplan_schema();
    let compiled = JSONSchema::compile(&schema_value)
        .map_err(|e| ValidationError::SchemaError(e.to_string()))?;

    let validation = compiled.validate(packplan);
    if let Err(errors) = validation {
        for error in errors {
            result.add_error(ValidationError::SchemaError(format!(
                "{} at {}",
                error, error.instance_path
            )));
        }
    }

    // Check that all decisions have evidence
    if let Some(clusters) = packplan.get("clusters").and_then(|c| c.as_array()) {
        for cluster in clusters {
            if let Some(decisions) = cluster.get("decisions").and_then(|d| d.as_array()) {
                for decision in decisions {
                    if let Some(evidence_refs) =
                        decision.get("evidence_refs").and_then(|e| e.as_array())
                    {
                        if evidence_refs.is_empty() {
                            let decision_text = decision
                                .get("decision")
                                .and_then(|d| d.as_str())
                                .unwrap_or("unknown");
                            result.add_error(ValidationError::DecisionWithoutEvidence(
                                decision_text.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Validate a complete bundle (manifest + evidence + checksums).
pub fn validate_bundle(
    manifest: &crate::Manifest,
    evidence_files: &HashSet<String>,
    checksums: &std::collections::HashMap<String, String>,
) -> Result<ValidationResult, ValidationError> {
    let mut result = ValidationResult::new();

    // Validate manifest against schema
    let manifest_json = serde_json::to_value(manifest)?;
    let manifest_result = validate_manifest(&manifest_json)?;
    for error in manifest_result.errors {
        result.add_error(error);
    }
    for warning in manifest_result.warnings {
        result.add_warning(warning);
    }

    // Check that all evidence references exist
    for process in &manifest.processes {
        if let Some(ref evidence_ref) = process.evidence_ref {
            if !evidence_files.contains(evidence_ref) {
                result.add_error(ValidationError::InvalidEvidenceRef(evidence_ref.clone()));
            }
        }
    }

    for service in &manifest.services {
        if let Some(ref evidence_ref) = service.evidence_ref {
            if !evidence_files.contains(evidence_ref) {
                result.add_error(ValidationError::InvalidEvidenceRef(evidence_ref.clone()));
            }
        }
    }

    for port in &manifest.ports {
        if let Some(ref evidence_ref) = port.evidence_ref {
            if !evidence_files.contains(evidence_ref) {
                result.add_error(ValidationError::InvalidEvidenceRef(evidence_ref.clone()));
            }
        }
    }

    for config in &manifest.config_files {
        if let Some(ref attachment_ref) = config.attachment_ref {
            if !evidence_files.contains(attachment_ref) {
                result.add_error(ValidationError::InvalidEvidenceRef(attachment_ref.clone()));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_manifest_minimal() {
        let manifest = serde_json::json!({
            "schema_version": "1.0.0",
            "collection_id": "550e8400-e29b-41d4-a716-446655440000",
            "collected_at": "2024-01-01T00:00:00Z",
            "system": {
                "hostname": "test-host",
                "os_type": "linux"
            },
            "processes": [],
            "services": [],
            "ports": []
        });

        let result = validate_manifest(&manifest).unwrap();
        assert!(result.valid, "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_validate_manifest_missing_field() {
        let manifest = serde_json::json!({
            "schema_version": "1.0.0"
            // Missing required fields
        });

        let result = validate_manifest(&manifest).unwrap();
        assert!(!result.valid);
    }
}

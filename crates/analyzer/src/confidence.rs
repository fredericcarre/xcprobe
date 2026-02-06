//! Confidence scoring and validation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use xcprobe_bundle_schema::{AppCluster, PackPlan};

/// Confidence report for a cluster.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfidenceReport {
    pub cluster_id: String,
    pub overall_confidence: f64,
    pub decisions: Vec<DecisionConfidence>,
    pub warnings: Vec<String>,
    pub missing_evidence: Vec<String>,
}

/// Confidence for a single decision.
#[derive(Debug, Serialize, Deserialize)]
pub struct DecisionConfidence {
    pub decision: String,
    pub confidence: f64,
    pub has_evidence: bool,
    pub evidence_refs: Vec<String>,
}

/// Result of plan validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub overall_confidence: f64,
    pub decisions_without_evidence: Vec<String>,
    pub total_decisions: usize,
    pub decisions_with_evidence: usize,
}

/// Calculate confidence score for a cluster.
pub fn calculate_cluster_confidence(cluster: &mut AppCluster) {
    if cluster.decisions.is_empty() {
        cluster.confidence = 0.0;
        return;
    }

    // Calculate weighted average confidence
    let mut total_confidence = 0.0;
    let mut total_weight = 0.0;

    for decision in &cluster.decisions {
        // Weight by whether decision has evidence
        let weight = if decision.evidence_refs.is_empty() {
            0.5 // Lower weight for decisions without evidence
        } else {
            1.0
        };

        total_confidence += decision.confidence * weight;
        total_weight += weight;
    }

    cluster.confidence = if total_weight > 0.0 {
        total_confidence / total_weight
    } else {
        0.0
    };

    // Penalize clusters with many decisions lacking evidence
    let evidence_ratio = cluster
        .decisions
        .iter()
        .filter(|d| !d.evidence_refs.is_empty())
        .count() as f64
        / cluster.decisions.len() as f64;

    cluster.confidence *= 0.5 + (evidence_ratio * 0.5);
}

/// Validate that a pack plan has evidence for all decisions.
pub fn validate_plan_evidence(plan: &PackPlan) -> ValidationResult {
    let mut decisions_without_evidence = Vec::new();
    let mut total_decisions = 0;
    let mut decisions_with_evidence = 0;
    let mut total_confidence = 0.0;

    for cluster in &plan.clusters {
        for decision in &cluster.decisions {
            total_decisions += 1;
            total_confidence += decision.confidence;

            if decision.evidence_refs.is_empty() {
                decisions_without_evidence.push(format!("[{}] {}", cluster.id, decision.decision));
            } else {
                decisions_with_evidence += 1;
            }
        }
    }

    let overall_confidence = if total_decisions > 0 {
        total_confidence / total_decisions as f64
    } else {
        0.0
    };

    ValidationResult {
        overall_confidence,
        decisions_without_evidence,
        total_decisions,
        decisions_with_evidence,
    }
}

/// Generate a confidence report for a cluster.
pub fn generate_confidence_report(cluster: &AppCluster) -> Result<String> {
    let mut missing_evidence = Vec::new();
    let decisions: Vec<DecisionConfidence> = cluster
        .decisions
        .iter()
        .map(|d| {
            let has_evidence = !d.evidence_refs.is_empty();
            if !has_evidence {
                missing_evidence.push(d.decision.clone());
            }
            DecisionConfidence {
                decision: d.decision.clone(),
                confidence: d.confidence,
                has_evidence,
                evidence_refs: d.evidence_refs.clone(),
            }
        })
        .collect();

    let mut warnings = Vec::new();
    if !missing_evidence.is_empty() {
        warnings.push(format!(
            "{} decisions lack evidence references",
            missing_evidence.len()
        ));
    }

    if cluster.confidence < 0.7 {
        warnings.push(format!(
            "Low overall confidence ({:.2}), manual review recommended",
            cluster.confidence
        ));
    }

    if cluster.ports.is_empty() && cluster.app_type != "worker" && cluster.app_type != "batch" {
        warnings.push("No network ports detected, verify if this is expected".to_string());
    }

    let report = ConfidenceReport {
        cluster_id: cluster.id.clone(),
        overall_confidence: cluster.confidence,
        decisions,
        warnings,
        missing_evidence,
    };

    Ok(serde_json::to_string_pretty(&report)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcprobe_bundle_schema::Decision;

    #[test]
    fn test_calculate_cluster_confidence() {
        let mut cluster = AppCluster {
            id: "test".to_string(),
            name: "test".to_string(),
            description: None,
            app_type: "api".to_string(),
            processes: vec![],
            services: vec![],
            ports: vec![],
            env_vars: vec![],
            config_files: vec![],
            log_paths: vec![],
            depends_on: vec![],
            external_deps: vec![],
            readiness: None,
            confidence: 0.0,
            evidence_refs: vec![],
            decisions: vec![
                Decision::new(
                    "Decision with evidence",
                    "Found in config",
                    vec!["evidence/test.txt".to_string()],
                    0.9,
                ),
                Decision::new("Decision without evidence", "Inferred", vec![], 0.6),
            ],
        };

        calculate_cluster_confidence(&mut cluster);

        // Confidence should be reasonable
        assert!(cluster.confidence > 0.5);
        assert!(cluster.confidence < 1.0);
    }

    #[test]
    fn test_validate_plan_evidence() {
        let plan = PackPlan {
            schema_version: "1.0.0".to_string(),
            generated_at: chrono::Utc::now(),
            source_bundle_id: "test".to_string(),
            clusters: vec![AppCluster {
                id: "test".to_string(),
                name: "test".to_string(),
                description: None,
                app_type: "api".to_string(),
                processes: vec![],
                services: vec![],
                ports: vec![],
                env_vars: vec![],
                config_files: vec![],
                log_paths: vec![],
                depends_on: vec![],
                external_deps: vec![],
                readiness: None,
                confidence: 0.8,
                evidence_refs: vec![],
                decisions: vec![
                    Decision::new(
                        "With evidence",
                        "reason",
                        vec!["evidence.txt".to_string()],
                        0.9,
                    ),
                    Decision::new("Without evidence", "reason", vec![], 0.5),
                ],
            }],
            external_dependencies: vec![],
            startup_dag: vec![],
            artifacts: vec![],
            overall_confidence: 0.8,
            warnings: vec![],
        };

        let result = validate_plan_evidence(&plan);

        assert_eq!(result.total_decisions, 2);
        assert_eq!(result.decisions_with_evidence, 1);
        assert_eq!(result.decisions_without_evidence.len(), 1);
    }
}

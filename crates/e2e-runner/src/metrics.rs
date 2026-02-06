//! Test metrics calculation.

use crate::truth::Truth;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use xcprobe_bundle_schema::PackPlan;

/// Metrics for a test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMetrics {
    /// Recall for process command lines
    pub process_cmdline_recall: f64,
    /// Precision for process command lines
    pub process_cmdline_precision: f64,
    /// Recall for ports
    pub ports_recall: f64,
    /// Precision for ports
    pub ports_precision: f64,
    /// Recall for environment variable names
    pub env_names_recall: f64,
    /// Precision for environment variable names
    pub env_names_precision: f64,
    /// Recall for dependencies
    pub deps_recall: f64,
    /// Precision for dependencies
    pub deps_precision: f64,
    /// Ratio of decisions with evidence
    pub decisions_with_evidence_ratio: f64,
    /// Total decisions
    pub total_decisions: usize,
    /// Decisions with evidence
    pub decisions_with_evidence: usize,
}

impl Default for TestMetrics {
    fn default() -> Self {
        Self {
            process_cmdline_recall: 0.0,
            process_cmdline_precision: 0.0,
            ports_recall: 0.0,
            ports_precision: 0.0,
            env_names_recall: 0.0,
            env_names_precision: 0.0,
            deps_recall: 0.0,
            deps_precision: 0.0,
            decisions_with_evidence_ratio: 0.0,
            total_decisions: 0,
            decisions_with_evidence: 0,
        }
    }
}

/// Calculate recall and precision.
fn calculate_recall_precision<T: std::hash::Hash + Eq + Clone>(
    expected: &[T],
    actual: &[T],
) -> (f64, f64) {
    if expected.is_empty() && actual.is_empty() {
        return (1.0, 1.0);
    }

    let expected_set: HashSet<_> = expected.iter().cloned().collect();
    let actual_set: HashSet<_> = actual.iter().cloned().collect();

    let true_positives = expected_set.intersection(&actual_set).count();

    let recall = if expected_set.is_empty() {
        1.0
    } else {
        true_positives as f64 / expected_set.len() as f64
    };

    let precision = if actual_set.is_empty() {
        1.0
    } else {
        true_positives as f64 / actual_set.len() as f64
    };

    (recall, precision)
}

/// Calculate metrics by comparing pack plan to truth.
pub fn calculate_metrics(plan: &PackPlan, truth: &Truth) -> TestMetrics {
    let mut metrics = TestMetrics::default();

    // Process/command line metrics
    let expected_cmds: Vec<String> = truth
        .applications
        .iter()
        .flat_map(|app| app.command_patterns.clone())
        .collect();

    let actual_cmds: Vec<String> = plan
        .clusters
        .iter()
        .flat_map(|c| {
            c.services
                .iter()
                .filter_map(|s| s.exec_start.clone())
                .chain(c.processes.iter().map(|p| p.command.clone()))
        })
        .collect();

    // For command matching, check if expected patterns are found in actual commands
    let mut matched_cmds = 0;
    for expected in &expected_cmds {
        if actual_cmds.iter().any(|actual| actual.contains(expected)) {
            matched_cmds += 1;
        }
    }

    metrics.process_cmdline_recall = if expected_cmds.is_empty() {
        1.0
    } else {
        matched_cmds as f64 / expected_cmds.len() as f64
    };

    metrics.process_cmdline_precision = if actual_cmds.is_empty() {
        1.0
    } else {
        matched_cmds as f64 / actual_cmds.len() as f64
    };

    // Port metrics
    let expected_ports: Vec<u16> = truth.ports.iter().map(|p| p.port).collect();
    let actual_ports: Vec<u16> = plan
        .clusters
        .iter()
        .flat_map(|c| c.ports.iter().map(|p| p.port))
        .collect();

    let (ports_recall, ports_precision) =
        calculate_recall_precision(&expected_ports, &actual_ports);
    metrics.ports_recall = ports_recall;
    metrics.ports_precision = ports_precision;

    // Environment variable metrics
    let expected_env: Vec<String> = truth.env_names.clone();
    let actual_env: Vec<String> = plan
        .clusters
        .iter()
        .flat_map(|c| c.env_vars.iter().map(|e| e.name.clone()))
        .collect();

    let (env_recall, env_precision) = calculate_recall_precision(&expected_env, &actual_env);
    metrics.env_names_recall = env_recall;
    metrics.env_names_precision = env_precision;

    // Dependency metrics
    let expected_deps: Vec<(String, String)> = truth
        .dependencies
        .iter()
        .map(|d| (d.from.clone(), d.to.clone()))
        .collect();

    let actual_deps: Vec<(String, String)> = plan
        .startup_dag
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();

    let (deps_recall, deps_precision) = calculate_recall_precision(&expected_deps, &actual_deps);
    metrics.deps_recall = deps_recall;
    metrics.deps_precision = deps_precision;

    // Evidence metrics
    let mut total_decisions = 0;
    let mut decisions_with_evidence = 0;

    for cluster in &plan.clusters {
        for decision in &cluster.decisions {
            total_decisions += 1;
            if !decision.evidence_refs.is_empty() {
                decisions_with_evidence += 1;
            }
        }
    }

    metrics.total_decisions = total_decisions;
    metrics.decisions_with_evidence = decisions_with_evidence;
    metrics.decisions_with_evidence_ratio = if total_decisions == 0 {
        1.0
    } else {
        decisions_with_evidence as f64 / total_decisions as f64
    };

    metrics
}

/// Check if metrics pass thresholds.
pub fn check_thresholds(metrics: &TestMetrics, truth: &Truth) -> Vec<String> {
    let mut failures = Vec::new();

    if metrics.process_cmdline_recall < truth.thresholds.process_cmdline_recall {
        failures.push(format!(
            "Process/cmdline recall {:.2}% < {:.2}%",
            metrics.process_cmdline_recall * 100.0,
            truth.thresholds.process_cmdline_recall * 100.0
        ));
    }

    if metrics.ports_recall < truth.thresholds.ports_recall {
        failures.push(format!(
            "Ports recall {:.2}% < {:.2}%",
            metrics.ports_recall * 100.0,
            truth.thresholds.ports_recall * 100.0
        ));
    }

    if metrics.env_names_recall < truth.thresholds.env_names_recall {
        failures.push(format!(
            "Env names recall {:.2}% < {:.2}%",
            metrics.env_names_recall * 100.0,
            truth.thresholds.env_names_recall * 100.0
        ));
    }

    if metrics.deps_recall < truth.thresholds.deps_recall {
        failures.push(format!(
            "Dependencies recall {:.2}% < {:.2}%",
            metrics.deps_recall * 100.0,
            truth.thresholds.deps_recall * 100.0
        ));
    }

    if truth.thresholds.require_all_evidence && metrics.decisions_with_evidence_ratio < 1.0 {
        failures.push(format!(
            "Decisions without evidence: {} of {}",
            metrics.total_decisions - metrics.decisions_with_evidence,
            metrics.total_decisions
        ));
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_recall_precision() {
        let expected = vec![1, 2, 3, 4, 5];
        let actual = vec![1, 2, 3, 6, 7];

        let (recall, precision) = calculate_recall_precision(&expected, &actual);

        // 3 matches out of 5 expected = 0.6 recall
        assert!((recall - 0.6).abs() < 0.01);
        // 3 matches out of 5 actual = 0.6 precision
        assert!((precision - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_empty_sets() {
        let expected: Vec<i32> = vec![];
        let actual: Vec<i32> = vec![];

        let (recall, precision) = calculate_recall_precision(&expected, &actual);

        assert_eq!(recall, 1.0);
        assert_eq!(precision, 1.0);
    }
}

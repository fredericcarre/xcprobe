//! Report generation.

use crate::runner::RunResult;
use anyhow::Result;
use std::path::Path;

/// Generate a report from test results.
pub fn generate_report(results_dir: &Path, format: &str) -> Result<()> {
    // Load all report.json files from the results directory
    let mut results = Vec::new();

    for entry in std::fs::read_dir(results_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let report_path = path.join("report.json");
            if report_path.exists() {
                let content = std::fs::read_to_string(&report_path)?;
                let result: RunResult = serde_json::from_str(&content)?;
                results.push(result);
            }
        }
    }

    match format {
        "json" => print_json_report(&results),
        "html" => print_html_report(&results),
        _ => print_text_report(&results),
    }

    Ok(())
}

fn print_text_report(results: &[RunResult]) {
    println!("=== XCProbe E2E Test Report ===\n");

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    println!("Summary:");
    println!("  Total:  {}", total);
    println!(
        "  Passed: {} ({:.1}%)",
        passed,
        (passed as f64 / total as f64) * 100.0
    );
    println!(
        "  Failed: {} ({:.1}%)",
        failed,
        (failed as f64 / total as f64) * 100.0
    );
    println!();

    // Aggregate metrics
    if !results.is_empty() {
        let avg_process_recall: f64 = results
            .iter()
            .map(|r| r.metrics.process_cmdline_recall)
            .sum::<f64>()
            / results.len() as f64;
        let avg_ports_recall: f64 =
            results.iter().map(|r| r.metrics.ports_recall).sum::<f64>() / results.len() as f64;
        let avg_env_recall: f64 = results
            .iter()
            .map(|r| r.metrics.env_names_recall)
            .sum::<f64>()
            / results.len() as f64;
        let avg_deps_recall: f64 =
            results.iter().map(|r| r.metrics.deps_recall).sum::<f64>() / results.len() as f64;
        let avg_evidence: f64 = results
            .iter()
            .map(|r| r.metrics.decisions_with_evidence_ratio)
            .sum::<f64>()
            / results.len() as f64;

        println!("Average Metrics:");
        println!(
            "  Process/Cmdline Recall: {:.1}%",
            avg_process_recall * 100.0
        );
        println!("  Ports Recall:           {:.1}%", avg_ports_recall * 100.0);
        println!("  Env Names Recall:       {:.1}%", avg_env_recall * 100.0);
        println!("  Dependencies Recall:    {:.1}%", avg_deps_recall * 100.0);
        println!("  Evidence Coverage:      {:.1}%", avg_evidence * 100.0);
        println!();
    }

    println!("Scenario Results:");
    println!("{:-<80}", "");
    println!(
        "{:<30} {:>10} {:>10} {:>10} {:>10}",
        "Scenario", "Status", "Proc%", "Port%", "Time(s)"
    );
    println!("{:-<80}", "");

    for result in results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        println!(
            "{:<30} {:>10} {:>10.1} {:>10.1} {:>10.2}",
            &result.scenario_name[..result.scenario_name.len().min(30)],
            status,
            result.metrics.process_cmdline_recall * 100.0,
            result.metrics.ports_recall * 100.0,
            result.duration_seconds
        );
    }
    println!("{:-<80}", "");

    // Print failures
    let failures: Vec<_> = results.iter().filter(|r| !r.passed).collect();
    if !failures.is_empty() {
        println!("\nFailures:");
        for result in failures {
            println!(
                "\n  {} ({:.2}s):",
                result.scenario_name, result.duration_seconds
            );
            for failure in &result.failures {
                println!("    - {}", failure);
            }
        }
    }
}

fn print_json_report(results: &[RunResult]) {
    let report = serde_json::json!({
        "summary": {
            "total": results.len(),
            "passed": results.iter().filter(|r| r.passed).count(),
            "failed": results.iter().filter(|r| !r.passed).count(),
        },
        "results": results,
    });

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn print_html_report(results: &[RunResult]) {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    println!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>XCProbe E2E Test Report</title>
    <style>
        body {{ font-family: sans-serif; margin: 20px; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #4CAF50; color: white; }}
        tr:nth-child(even) {{ background-color: #f2f2f2; }}
        .pass {{ color: green; font-weight: bold; }}
        .fail {{ color: red; font-weight: bold; }}
        .summary {{ margin-bottom: 20px; }}
    </style>
</head>
<body>
    <h1>XCProbe E2E Test Report</h1>

    <div class="summary">
        <h2>Summary</h2>
        <p>Total: {total} | Passed: {passed} | Failed: {failed}</p>
    </div>

    <h2>Results</h2>
    <table>
        <tr>
            <th>Scenario</th>
            <th>Status</th>
            <th>Process Recall</th>
            <th>Ports Recall</th>
            <th>Env Recall</th>
            <th>Deps Recall</th>
            <th>Evidence</th>
            <th>Duration</th>
        </tr>"#
    );

    for result in results {
        let status_class = if result.passed { "pass" } else { "fail" };
        let status_text = if result.passed { "PASS" } else { "FAIL" };

        println!(
            r#"        <tr>
            <td>{}</td>
            <td class="{}">{}</td>
            <td>{:.1}%</td>
            <td>{:.1}%</td>
            <td>{:.1}%</td>
            <td>{:.1}%</td>
            <td>{:.1}%</td>
            <td>{:.2}s</td>
        </tr>"#,
            result.scenario_name,
            status_class,
            status_text,
            result.metrics.process_cmdline_recall * 100.0,
            result.metrics.ports_recall * 100.0,
            result.metrics.env_names_recall * 100.0,
            result.metrics.deps_recall * 100.0,
            result.metrics.decisions_with_evidence_ratio * 100.0,
            result.duration_seconds
        );
    }

    println!(
        r#"    </table>
</body>
</html>"#
    );
}

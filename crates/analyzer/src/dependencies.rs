//! Dependency detection and DAG building.

use anyhow::Result;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use xcprobe_bundle_schema::{AppCluster, Bundle, DagEdge, Decision, DependencyInfo};

/// Pattern to detect connection strings and endpoints.
static ENDPOINT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:",
        r"(?:mongodb|mysql|postgres|postgresql|redis|amqp|http|https)://[^\s]+",
        r"|(?:host|hostname|server|endpoint)\s*[=:]\s*[^\s,]+",
        r"|(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})(?::(\d+))?",
        r")"
    ))
    .unwrap()
});

/// Pattern to detect database/cache hostnames.
static DB_HOST_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:database|db|redis|cache|mongo|postgres|mysql|rabbit|kafka)",
        r"[-_]?(?:host|server|endpoint|url)\s*[=:]\s*([^\s,]+)"
    ))
    .unwrap()
});

/// Detect dependencies for clusters.
pub fn detect_dependencies(bundle: &Bundle, clusters: &mut [AppCluster]) -> Result<()> {
    let mut external_deps: Vec<DependencyInfo> = Vec::new();
    let mut dep_id = 0;

    // Build a map of ports to cluster IDs for internal dependency detection
    let mut port_to_cluster: HashMap<u16, String> = HashMap::new();
    for cluster in clusters.iter() {
        for port in &cluster.ports {
            port_to_cluster.insert(port.port, cluster.id.clone());
        }
    }

    for cluster in clusters.iter_mut() {
        // Scan config files for endpoints
        for config in &cluster.config_files {
            if let Some(ref evidence_ref) = config.evidence_ref {
                if let Some(evidence) = bundle.evidence.get(evidence_ref) {
                    if let Some(ref content) = evidence.content {
                        let content_str = String::from_utf8_lossy(content);

                        // Find endpoints in config
                        for caps in ENDPOINT_PATTERN.captures_iter(&content_str) {
                            let endpoint = caps.get(0).map(|m| m.as_str()).unwrap_or("");

                            // Try to extract port
                            let port = extract_port_from_endpoint(endpoint);

                            // Check if this is an internal dependency
                            if let Some(port_num) = port {
                                if let Some(dep_cluster_id) = port_to_cluster.get(&port_num) {
                                    if dep_cluster_id != &cluster.id {
                                        // Internal dependency
                                        if !cluster.depends_on.contains(dep_cluster_id) {
                                            cluster.depends_on.push(dep_cluster_id.clone());
                                            cluster.decisions.push(Decision::new(
                                                format!("Depends on cluster {} (port {})", dep_cluster_id, port_num),
                                                format!("Found endpoint {} in config {}", endpoint, config.source_path),
                                                vec![evidence_ref.clone()],
                                                0.9,
                                            ));
                                        }
                                        continue;
                                    }
                                }
                            }

                            // External dependency
                            let dep_type = detect_dependency_type(endpoint, port);
                            let dep = DependencyInfo {
                                id: format!("ext-{}", dep_id),
                                dep_type,
                                endpoint: endpoint.to_string(),
                                port,
                                used_by: vec![cluster.id.clone()],
                                evidence_refs: vec![evidence_ref.clone()],
                            };

                            cluster.external_deps.push(dep.id.clone());
                            cluster.decisions.push(Decision::new(
                                format!("External dependency detected: {}", endpoint),
                                format!("Found in config file: {}", config.source_path),
                                vec![evidence_ref.clone()],
                                0.8,
                            ));

                            external_deps.push(dep);
                            dep_id += 1;
                        }

                        // Also check for DB host patterns
                        for caps in DB_HOST_PATTERN.captures_iter(&content_str) {
                            if let Some(host) = caps.get(1) {
                                let host_str = host.as_str();

                                // Skip localhost
                                if host_str == "localhost" || host_str == "127.0.0.1" {
                                    continue;
                                }

                                let dep = DependencyInfo {
                                    id: format!("ext-{}", dep_id),
                                    dep_type: "database".to_string(),
                                    endpoint: host_str.to_string(),
                                    port: None,
                                    used_by: vec![cluster.id.clone()],
                                    evidence_refs: vec![evidence_ref.clone()],
                                };

                                cluster.external_deps.push(dep.id.clone());
                                cluster.decisions.push(Decision::new(
                                    format!("Database dependency detected: {}", host_str),
                                    format!("Found DB_HOST pattern in config: {}", config.source_path),
                                    vec![evidence_ref.clone()],
                                    0.85,
                                ));

                                external_deps.push(dep);
                                dep_id += 1;
                            }
                        }
                    }
                }
            }
        }

        // Scan environment variables for common dependency patterns
        for env_var in &cluster.env_vars {
            let name_lower = env_var.name.to_lowercase();

            // Check for dependency-related env vars
            let dep_patterns = [
                ("database", &["database_url", "db_url", "db_host", "postgres", "mysql"]),
                ("cache", &["redis_url", "redis_host", "cache_url", "memcached"]),
                ("messagequeue", &["amqp_url", "rabbitmq", "kafka"]),
                ("api", &["api_url", "api_host", "service_url"]),
            ];

            for (dep_type, patterns) in dep_patterns {
                if patterns.iter().any(|p| name_lower.contains(p)) {
                    cluster.decisions.push(Decision::new(
                        format!("Likely {} dependency from env var {}", dep_type, env_var.name),
                        "Environment variable name suggests external dependency",
                        env_var.evidence_ref.iter().cloned().collect(),
                        0.7,
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Extract port from an endpoint string.
fn extract_port_from_endpoint(endpoint: &str) -> Option<u16> {
    // Check for port in URL
    if let Some(idx) = endpoint.rfind(':') {
        let port_str = &endpoint[idx + 1..];
        let port_str = port_str.split('/').next().unwrap_or(port_str);
        let port_str = port_str.split('?').next().unwrap_or(port_str);
        if let Ok(port) = port_str.parse() {
            return Some(port);
        }
    }

    // Default ports for known schemes
    if endpoint.starts_with("postgres://") || endpoint.starts_with("postgresql://") {
        return Some(5432);
    } else if endpoint.starts_with("mysql://") {
        return Some(3306);
    } else if endpoint.starts_with("redis://") {
        return Some(6379);
    } else if endpoint.starts_with("mongodb://") {
        return Some(27017);
    } else if endpoint.starts_with("amqp://") {
        return Some(5672);
    } else if endpoint.starts_with("http://") {
        return Some(80);
    } else if endpoint.starts_with("https://") {
        return Some(443);
    }

    None
}

/// Detect the type of dependency from endpoint.
fn detect_dependency_type(endpoint: &str, port: Option<u16>) -> String {
    let endpoint_lower = endpoint.to_lowercase();

    // Check URL scheme
    if endpoint_lower.starts_with("postgres") || endpoint_lower.starts_with("mysql") {
        return "database".to_string();
    } else if endpoint_lower.starts_with("redis") || endpoint_lower.starts_with("memcached") {
        return "cache".to_string();
    } else if endpoint_lower.starts_with("amqp") || endpoint_lower.contains("rabbit") || endpoint_lower.contains("kafka") {
        return "messagequeue".to_string();
    } else if endpoint_lower.starts_with("mongodb") {
        return "database".to_string();
    }

    // Check port
    match port {
        Some(5432) => "database".to_string(),
        Some(3306) => "database".to_string(),
        Some(27017) => "database".to_string(),
        Some(6379) => "cache".to_string(),
        Some(11211) => "cache".to_string(),
        Some(5672) | Some(15672) => "messagequeue".to_string(),
        Some(9092) => "messagequeue".to_string(),
        Some(9200) | Some(9300) => "search".to_string(),
        Some(80) | Some(443) | Some(8080) => "api".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Build startup DAG based on dependencies.
pub fn build_startup_dag(clusters: &[AppCluster]) -> Vec<DagEdge> {
    let mut graph: DiGraph<String, ()> = DiGraph::new();
    let mut node_map: HashMap<String, NodeIndex> = HashMap::new();

    // Add nodes
    for cluster in clusters {
        let idx = graph.add_node(cluster.id.clone());
        node_map.insert(cluster.id.clone(), idx);
    }

    // Add edges (from dependency to dependent)
    let mut edges = Vec::new();
    for cluster in clusters {
        if let Some(&to_idx) = node_map.get(&cluster.id) {
            for dep_id in &cluster.depends_on {
                if let Some(&from_idx) = node_map.get(dep_id) {
                    graph.add_edge(from_idx, to_idx, ());
                    edges.push(DagEdge {
                        from: dep_id.clone(),
                        to: cluster.id.clone(),
                        reason: format!("Cluster {} depends on {}", cluster.id, dep_id),
                    });
                }
            }
        }
    }

    // Check for cycles
    if toposort(&graph, None).is_err() {
        tracing::warn!("Circular dependencies detected in startup DAG");
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_port_from_endpoint() {
        assert_eq!(extract_port_from_endpoint("postgres://localhost:5432/db"), Some(5432));
        assert_eq!(extract_port_from_endpoint("redis://cache:6379"), Some(6379));
        assert_eq!(extract_port_from_endpoint("http://api.example.com:8080/v1"), Some(8080));
        assert_eq!(extract_port_from_endpoint("postgres://db/mydb"), Some(5432));
        assert_eq!(extract_port_from_endpoint("https://api.example.com"), Some(443));
    }

    #[test]
    fn test_detect_dependency_type() {
        assert_eq!(detect_dependency_type("postgres://localhost:5432", Some(5432)), "database");
        assert_eq!(detect_dependency_type("redis://cache:6379", Some(6379)), "cache");
        assert_eq!(detect_dependency_type("amqp://rabbit:5672", Some(5672)), "messagequeue");
        assert_eq!(detect_dependency_type("192.168.1.100", Some(80)), "api");
    }
}

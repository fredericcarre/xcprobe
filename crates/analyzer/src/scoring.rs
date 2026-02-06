//! Process/service scoring for business relevance.

use std::collections::HashMap;
use xcprobe_bundle_schema::Manifest;

/// Score for a process or service.
#[derive(Debug, Clone)]
pub struct ProcessScore {
    pub pid: u32,
    pub name: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub is_business_process: bool,
}

/// Score processes for business relevance.
///
/// Higher scores indicate more likely to be a business application.
/// System processes, kernel threads, etc. get lower scores.
pub fn score_processes(manifest: &Manifest) -> HashMap<u32, ProcessScore> {
    let mut scores = HashMap::new();

    // Score based on process characteristics
    for process in &manifest.processes {
        let mut score: f64 = 0.5; // Base score
        let mut reasons = Vec::new();

        // System process detection (lower score)
        let system_prefixes = [
            "kworker",
            "migration",
            "ksoftirqd",
            "rcu_",
            "watchdog",
            "kthreadd",
            "kswapd",
            "khugepaged",
            "kcompactd",
        ];
        if system_prefixes
            .iter()
            .any(|p| process.command.starts_with(p))
        {
            score = 0.1;
            reasons.push("System kernel thread".to_string());
        }

        // Container/orchestration processes
        let container_keywords = ["docker", "containerd", "kubelet", "crio"];
        if container_keywords
            .iter()
            .any(|k| process.command.contains(k))
        {
            score = 0.3;
            reasons.push("Container runtime process".to_string());
        }

        // Common service processes (higher score)
        let service_keywords = [
            "nginx", "apache", "httpd", "java", "python", "node", "ruby", "php", "dotnet",
            "postgres", "mysql", "redis", "mongo", "rabbit", "kafka", "elastic",
        ];
        if service_keywords.iter().any(|k| {
            process.command.to_lowercase().contains(k)
                || process.full_cmdline.to_lowercase().contains(k)
        }) {
            score = 0.8;
            reasons.push("Known application framework".to_string());
        }

        // Listening on a port is a strong signal
        if let Some(pid) = Some(process.pid) {
            if manifest.ports.iter().any(|p| p.pid == Some(pid)) {
                score = score.max(0.7);
                reasons.push("Listening on network port".to_string());
            }
        }

        // Has a corresponding systemd service
        for service in &manifest.services {
            if service.main_pid == Some(process.pid) {
                score = score.max(0.8);
                reasons.push(format!("Managed by systemd service: {}", service.name));
            }
        }

        // User processes (not root) often indicate business apps
        if process.user != "root"
            && !["nobody", "daemon", "systemd-network"].contains(&process.user.as_str())
        {
            score += 0.1;
            reasons.push(format!("Runs as user: {}", process.user));
        }

        // Long command lines suggest configuration
        if process.full_cmdline.len() > 100 {
            score += 0.05;
            reasons.push("Has complex command line".to_string());
        }

        let is_business = score >= 0.6;

        scores.insert(
            process.pid,
            ProcessScore {
                pid: process.pid,
                name: process.command.clone(),
                score,
                reasons,
                is_business_process: is_business,
            },
        );
    }

    scores
}

/// Score services for business relevance.
pub fn score_services(manifest: &Manifest) -> HashMap<String, f64> {
    let mut scores = HashMap::new();

    // System service patterns (lower score)
    let system_patterns = [
        "systemd-",
        "dbus",
        "polkit",
        "udev",
        "getty",
        "sshd",
        "cron",
        "rsyslog",
        "auditd",
        "firewalld",
        "networkmanager",
    ];

    for service in &manifest.services {
        let mut score: f64 = 0.5;

        // Check system patterns
        let name_lower = service.name.to_lowercase();
        if system_patterns.iter().any(|p| name_lower.contains(p)) {
            score = 0.2;
        }

        // Services with custom working directories
        if service.working_directory.is_some() {
            score = score.max(0.6);
        }

        // Services with environment files
        if !service.environment_files.is_empty() {
            score = score.max(0.7);
        }

        // Services running as non-root
        if let Some(ref user) = service.user {
            if user != "root" {
                score = score.max(0.7);
            }
        }

        scores.insert(service.name.clone(), score);
    }

    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_system_processes() {
        let mut manifest = Manifest::default();
        manifest.processes.push(xcprobe_bundle_schema::ProcessInfo {
            pid: 1,
            ppid: 0,
            user: "root".to_string(),
            command: "kworker/0:1".to_string(),
            args: vec![],
            full_cmdline: "[kworker/0:1]".to_string(),
            start_time: None,
            elapsed_time: None,
            cpu_percent: None,
            memory_percent: None,
            working_directory: None,
            environment: None,
            evidence_ref: None,
        });

        let scores = score_processes(&manifest);
        let score = scores.get(&1).unwrap();
        assert!(score.score < 0.5);
        assert!(!score.is_business_process);
    }

    #[test]
    fn test_score_application_processes() {
        let mut manifest = Manifest::default();
        manifest.processes.push(xcprobe_bundle_schema::ProcessInfo {
            pid: 1234,
            ppid: 1,
            user: "www-data".to_string(),
            command: "nginx".to_string(),
            args: vec!["-g".to_string(), "daemon off;".to_string()],
            full_cmdline: "nginx -g daemon off;".to_string(),
            start_time: None,
            elapsed_time: None,
            cpu_percent: None,
            memory_percent: None,
            working_directory: None,
            environment: None,
            evidence_ref: None,
        });
        manifest.ports.push(xcprobe_bundle_schema::PortInfo {
            protocol: "tcp".to_string(),
            local_address: "0.0.0.0".to_string(),
            local_port: 80,
            state: "LISTEN".to_string(),
            pid: Some(1234),
            process_name: Some("nginx".to_string()),
            evidence_ref: None,
        });

        let scores = score_processes(&manifest);
        let score = scores.get(&1234).unwrap();
        assert!(score.score >= 0.7);
        assert!(score.is_business_process);
    }
}

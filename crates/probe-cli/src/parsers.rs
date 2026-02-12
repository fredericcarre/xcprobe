//! Parsers for command outputs.

use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use xcprobe_bundle_schema::{Package, PortInfo, ProcessInfo, ScheduledTask, ServiceInfo};
use xcprobe_common::OsType;

/// Parse process list output.
pub fn parse_processes(output: &str, os_type: OsType) -> Result<Vec<ProcessInfo>> {
    match os_type {
        OsType::Linux => parse_linux_processes(output),
        OsType::Windows => parse_windows_processes(output),
    }
}

fn parse_linux_processes(output: &str) -> Result<Vec<ProcessInfo>> {
    let mut processes = Vec::new();

    for line in output.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 11 {
            continue;
        }

        // ps auxww format: USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND...
        let user = parts[0].to_string();
        let pid: u32 = parts[1].parse().unwrap_or(0);
        let cpu_percent: f32 = parts[2].parse().unwrap_or(0.0);
        let memory_percent: f32 = parts[3].parse().unwrap_or(0.0);

        // Command and args are from position 10 onwards
        let full_cmdline = parts[10..].join(" ");
        let command = parts.get(10).map(|s| s.to_string()).unwrap_or_default();
        let args: Vec<String> = parts[11..].iter().map(|s| s.to_string()).collect();

        processes.push(ProcessInfo {
            pid,
            ppid: 0, // Not available in ps aux
            user,
            command,
            args,
            full_cmdline,
            start_time: None,
            elapsed_time: None,
            cpu_percent: Some(cpu_percent),
            memory_percent: Some(memory_percent),
            working_directory: None,
            environment: None,
            evidence_ref: None,
        });
    }

    Ok(processes)
}

fn parse_windows_processes(output: &str) -> Result<Vec<ProcessInfo>> {
    // Windows output is JSON from PowerShell
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut processes = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            let pid = item["ProcessId"].as_u64().unwrap_or(0) as u32;
            let ppid = item["ParentProcessId"].as_u64().unwrap_or(0) as u32;
            let name = item["Name"].as_str().unwrap_or("").to_string();
            let cmdline = item["CommandLine"].as_str().unwrap_or("").to_string();

            processes.push(ProcessInfo {
                pid,
                ppid,
                user: String::new(),
                command: name.clone(),
                args: vec![],
                full_cmdline: cmdline,
                start_time: None,
                elapsed_time: None,
                cpu_percent: None,
                memory_percent: None,
                working_directory: None,
                environment: None,
                evidence_ref: None,
            });
        }
    }

    Ok(processes)
}

/// Parse service list output.
pub fn parse_service_list(output: &str, os_type: OsType) -> Result<Vec<String>> {
    match os_type {
        OsType::Linux => parse_linux_service_list(output),
        OsType::Windows => parse_windows_service_list(output),
    }
}

fn parse_linux_service_list(output: &str) -> Result<Vec<String>> {
    let mut services = Vec::new();

    for line in output.lines() {
        // Format: unit.service loaded active running Description
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(unit) = parts.first() {
            if unit.ends_with(".service") {
                services.push(unit.to_string());
            }
        }
    }

    Ok(services)
}

fn parse_windows_service_list(output: &str) -> Result<Vec<String>> {
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut services = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            if let Some(name) = item["Name"].as_str() {
                services.push(name.to_string());
            }
        }
    }

    Ok(services)
}

/// Parse full service list with details (Windows only).
/// The Windows list command already returns all fields we need, so we can
/// skip the per-service queries entirely.
pub fn parse_windows_services_from_list(output: &str) -> Result<Vec<ServiceInfo>> {
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut services = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            let name = item["Name"].as_str().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            services.push(ServiceInfo {
                name,
                display_name: item["DisplayName"].as_str().map(|s| s.to_string()),
                description: item["Description"].as_str().map(|s| s.to_string()),
                state: item["State"].as_str().unwrap_or("").to_string(),
                sub_state: None,
                start_mode: item["StartMode"].as_str().map(|s| s.to_string()),
                exec_start: item["PathName"].as_str().map(|s| s.to_string()),
                exec_start_pre: vec![],
                exec_start_post: vec![],
                exec_stop: None,
                working_directory: None,
                user: None,
                group: None,
                environment: HashMap::new(),
                environment_files: vec![],
                unit_file_path: None,
                dependencies: vec![],
                wanted_by: vec![],
                main_pid: None,
                evidence_ref: None,
            });
        }
    }

    Ok(services)
}

/// Parse service details.
pub fn parse_service_details(output: &str, os_type: OsType) -> Result<ServiceInfo> {
    match os_type {
        OsType::Linux => parse_linux_service_details(output),
        OsType::Windows => parse_windows_service_details(output),
    }
}

fn parse_linux_service_details(output: &str) -> Result<ServiceInfo> {
    let mut service = ServiceInfo {
        name: String::new(),
        display_name: None,
        description: None,
        state: String::new(),
        sub_state: None,
        start_mode: None,
        exec_start: None,
        exec_start_pre: vec![],
        exec_start_post: vec![],
        exec_stop: None,
        working_directory: None,
        user: None,
        group: None,
        environment: HashMap::new(),
        environment_files: vec![],
        unit_file_path: None,
        dependencies: vec![],
        wanted_by: vec![],
        main_pid: None,
        evidence_ref: None,
    };

    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0];
        let value = parts[1].to_string();

        match key {
            "Id" => service.name = value,
            "Description" => service.description = Some(value),
            "ActiveState" => service.state = value,
            "SubState" => service.sub_state = Some(value),
            "ExecStart" => service.exec_start = Some(value),
            "WorkingDirectory" => {
                if !value.is_empty() {
                    service.working_directory = Some(value);
                }
            }
            "User" => service.user = Some(value),
            "Group" => service.group = Some(value),
            "MainPID" => service.main_pid = value.parse().ok(),
            "FragmentPath" => service.unit_file_path = Some(value),
            _ => {}
        }
    }

    Ok(service)
}

fn parse_windows_service_details(output: &str) -> Result<ServiceInfo> {
    let json: serde_json::Value = serde_json::from_str(output)?;

    Ok(ServiceInfo {
        name: json["Name"].as_str().unwrap_or("").to_string(),
        display_name: json["DisplayName"].as_str().map(|s| s.to_string()),
        description: json["Description"].as_str().map(|s| s.to_string()),
        state: json["State"].as_str().unwrap_or("").to_string(),
        sub_state: None,
        start_mode: json["StartMode"].as_str().map(|s| s.to_string()),
        exec_start: json["PathName"].as_str().map(|s| s.to_string()),
        exec_start_pre: vec![],
        exec_start_post: vec![],
        exec_stop: None,
        working_directory: None,
        user: None,
        group: None,
        environment: HashMap::new(),
        environment_files: vec![],
        unit_file_path: None,
        dependencies: vec![],
        wanted_by: vec![],
        main_pid: None,
        evidence_ref: None,
    })
}

/// Parse systemd unit file content.
pub struct UnitFileInfo {
    pub exec_start: Option<String>,
    pub working_directory: Option<String>,
    pub environment_files: Vec<String>,
    pub environment: HashMap<String, String>,
}

pub fn parse_systemd_unit(content: &str) -> UnitFileInfo {
    let mut info = UnitFileInfo {
        exec_start: None,
        working_directory: None,
        environment_files: vec![],
        environment: HashMap::new(),
    };

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("ExecStart=") {
            info.exec_start = Some(line.trim_start_matches("ExecStart=").to_string());
        } else if line.starts_with("WorkingDirectory=") {
            info.working_directory = Some(line.trim_start_matches("WorkingDirectory=").to_string());
        } else if line.starts_with("EnvironmentFile=") {
            let path = line
                .trim_start_matches("EnvironmentFile=")
                .trim_start_matches('-');
            info.environment_files.push(path.to_string());
        } else if line.starts_with("Environment=") {
            let env = line.trim_start_matches("Environment=");
            if let Some((key, value)) = env.split_once('=') {
                info.environment
                    .insert(key.to_string(), value.trim_matches('"').to_string());
            }
        }
    }

    info
}

/// Parse ports/listeners output.
pub fn parse_ports(output: &str, os_type: OsType) -> Result<Vec<PortInfo>> {
    match os_type {
        OsType::Linux => parse_linux_ports(output),
        OsType::Windows => parse_windows_ports(output),
    }
}

fn parse_linux_ports(output: &str) -> Result<Vec<PortInfo>> {
    let mut ports = Vec::new();
    // Pattern for ss -lntup output:
    //   Netid  State  Recv-Q  Send-Q  Local Address:Port  Peer Address:Port  Process
    //   tcp    LISTEN 0       128     0.0.0.0:8080        0.0.0.0:*          users:(("python3",pid=7,fd=3))
    let re = Regex::new(concat!(
        r"(?P<proto>tcp|udp)\s+(?P<state>\w+)\s+\d+\s+\d+\s+",
        r"(?P<local>\S+):(?P<port>\d+)\s+\S+:\S+\s*",
        r#"(?:users:\(\("(?P<name>[^"]+)",pid=(?P<pid>\d+))?"#,
    ))?;

    for line in output.lines().skip(1) {
        if let Some(caps) = re.captures(line) {
            let port: u16 = caps
                .name("port")
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            if port == 0 {
                continue;
            }

            ports.push(PortInfo {
                protocol: caps
                    .name("proto")
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
                local_address: caps
                    .name("local")
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
                local_port: port,
                state: caps
                    .name("state")
                    .map(|m| m.as_str().to_string())
                    .unwrap_or("LISTEN".to_string()),
                pid: caps.name("pid").and_then(|m| m.as_str().parse().ok()),
                process_name: caps.name("name").map(|m| m.as_str().to_string()),
                evidence_ref: None,
            });
        }
    }

    Ok(ports)
}

fn parse_windows_ports(output: &str) -> Result<Vec<PortInfo>> {
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut ports = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            ports.push(PortInfo {
                protocol: "tcp".to_string(),
                local_address: item["LocalAddress"].as_str().unwrap_or("").to_string(),
                local_port: item["LocalPort"].as_u64().unwrap_or(0) as u16,
                state: item["State"].as_str().unwrap_or("").to_string(),
                pid: item["OwningProcess"].as_u64().map(|p| p as u32),
                process_name: None,
                evidence_ref: None,
            });
        }
    }

    Ok(ports)
}

/// Parse package list output.
pub fn parse_packages(output: &str, os_type: OsType, command: &str) -> Result<Vec<Package>> {
    match os_type {
        OsType::Linux => {
            if command.contains("dpkg") {
                parse_dpkg_packages(output)
            } else {
                parse_rpm_packages(output)
            }
        }
        OsType::Windows => parse_windows_packages(output),
    }
}

fn parse_dpkg_packages(output: &str) -> Result<Vec<Package>> {
    let mut packages = Vec::new();

    for line in output.lines() {
        if !line.starts_with("ii") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            packages.push(Package {
                name: parts[1].to_string(),
                version: parts[2].to_string(),
                architecture: parts.get(3).map(|s| s.to_string()),
                description: None,
                install_date: None,
                source: "dpkg".to_string(),
            });
        }
    }

    Ok(packages)
}

fn parse_rpm_packages(output: &str) -> Result<Vec<Package>> {
    let mut packages = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            packages.push(Package {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
                architecture: parts.get(2).map(|s| s.to_string()),
                description: None,
                install_date: None,
                source: "rpm".to_string(),
            });
        }
    }

    Ok(packages)
}

fn parse_windows_packages(output: &str) -> Result<Vec<Package>> {
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut packages = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            packages.push(Package {
                name: item["Name"].as_str().unwrap_or("").to_string(),
                version: item["Version"].as_str().unwrap_or("").to_string(),
                architecture: None,
                description: None,
                install_date: None,
                source: "windows".to_string(),
            });
        }
    }

    Ok(packages)
}

/// Parse scheduled tasks output.
pub fn parse_scheduled_tasks(output: &str, os_type: OsType) -> Result<Vec<ScheduledTask>> {
    match os_type {
        OsType::Linux => parse_linux_scheduled_tasks(output),
        OsType::Windows => parse_windows_scheduled_tasks(output),
    }
}

fn parse_linux_scheduled_tasks(output: &str) -> Result<Vec<ScheduledTask>> {
    let mut tasks = Vec::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            tasks.push(ScheduledTask {
                name: parts[0].to_string(),
                task_type: "systemd-timer".to_string(),
                schedule: parts.get(1).map(|s| s.to_string()),
                command: None,
                user: None,
                enabled: true,
                last_run: None,
                next_run: None,
                evidence_ref: None,
            });
        }
    }

    Ok(tasks)
}

fn parse_windows_scheduled_tasks(output: &str) -> Result<Vec<ScheduledTask>> {
    let json: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::Array(vec![]));

    let mut tasks = Vec::new();

    if let Some(array) = json.as_array() {
        for item in array {
            tasks.push(ScheduledTask {
                name: item["TaskName"].as_str().unwrap_or("").to_string(),
                task_type: "windows-task".to_string(),
                schedule: None,
                command: None,
                user: None,
                enabled: item["State"]
                    .as_str()
                    .map(|s| s == "Ready")
                    .unwrap_or(false),
                last_run: None,
                next_run: None,
                evidence_ref: None,
            });
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_linux_processes() {
        let output = r#"USER       PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND
root         1  0.0  0.1 169936 11892 ?        Ss   Jan01   0:05 /sbin/init
www-data  1234  0.5  1.2 123456 12345 ?        Sl   Jan01   1:23 nginx: worker process
"#;
        let procs = parse_linux_processes(output).unwrap();
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].user, "root");
        assert_eq!(procs[0].pid, 1);
        assert_eq!(procs[1].command, "nginx:");
    }

    #[test]
    fn test_parse_linux_ports() {
        let output = r#"Netid State  Recv-Q Send-Q   Local Address:Port   Peer Address:Port  Process
tcp   LISTEN 0      128        0.0.0.0:8080        0.0.0.0:*      users:(("python3",pid=7,fd=3))
tcp   LISTEN 0      128        0.0.0.0:8081        0.0.0.0:*      users:(("python3",pid=12,fd=4))
udp   UNCONN 0      0          0.0.0.0:5353        0.0.0.0:*
"#;
        let ports = parse_linux_ports(output).unwrap();
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0].local_port, 8080);
        assert_eq!(ports[0].protocol, "tcp");
        assert_eq!(ports[0].pid, Some(7));
        assert_eq!(ports[0].process_name, Some("python3".to_string()));
        assert_eq!(ports[1].local_port, 8081);
        assert_eq!(ports[1].pid, Some(12));
        assert_eq!(ports[2].local_port, 5353);
        assert_eq!(ports[2].protocol, "udp");
        assert_eq!(ports[2].pid, None);
    }

    #[test]
    fn test_parse_systemd_unit() {
        let content = r#"
[Unit]
Description=My Service

[Service]
ExecStart=/usr/bin/myapp --config /etc/myapp.conf
WorkingDirectory=/opt/myapp
EnvironmentFile=/etc/default/myapp
Environment=NODE_ENV=production
"#;
        let info = parse_systemd_unit(content);
        assert_eq!(
            info.exec_start,
            Some("/usr/bin/myapp --config /etc/myapp.conf".to_string())
        );
        assert_eq!(info.working_directory, Some("/opt/myapp".to_string()));
        assert_eq!(info.environment_files, vec!["/etc/default/myapp"]);
        assert_eq!(
            info.environment.get("NODE_ENV"),
            Some(&"production".to_string())
        );
    }
}

//! Allowlisted commands for collection.
//!
//! All commands that can be executed on targets are defined here.
//! This is the single source of truth for what the collector can run.

/// Trait for command sets.
pub trait CommandSet: Send + Sync {
    /// Get hostname command.
    fn hostname_cmd(&self) -> &str;

    /// Get OS version command.
    fn os_version_cmd(&self) -> Option<&str>;

    /// Get kernel version command.
    fn kernel_version_cmd(&self) -> Option<&str>;

    /// Get architecture command.
    fn architecture_cmd(&self) -> Option<&str>;

    /// Get process listing commands.
    fn process_cmds(&self) -> Vec<&str>;

    /// Get service listing command.
    fn service_list_cmd(&self) -> &str;

    /// Get service show command for a specific service.
    fn service_show_cmd(&self, name: &str) -> Option<String>;

    /// Get service cat command (for unit file content).
    fn service_cat_cmd(&self, name: &str) -> Option<String>;

    /// Get ports/listeners command.
    fn ports_cmd(&self) -> &str;

    /// Get package listing commands.
    fn package_cmds(&self) -> Vec<&str>;

    /// Get scheduled task listing commands.
    fn scheduled_task_cmds(&self) -> Vec<&str>;

    /// Get command to read a file.
    fn read_file_cmd(&self, path: &str) -> Option<String>;

    /// Get journal/event log command.
    fn journal_cmd(&self, unit: &str, since: &str) -> Option<String>;
}

/// Linux commands using standard tools.
pub struct LinuxCommands;

impl LinuxCommands {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxCommands {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSet for LinuxCommands {
    fn hostname_cmd(&self) -> &str {
        "hostname"
    }

    fn os_version_cmd(&self) -> Option<&str> {
        Some("cat /etc/os-release 2>/dev/null || cat /etc/redhat-release 2>/dev/null || cat /etc/debian_version 2>/dev/null")
    }

    fn kernel_version_cmd(&self) -> Option<&str> {
        Some("uname -r")
    }

    fn architecture_cmd(&self) -> Option<&str> {
        Some("uname -m")
    }

    fn process_cmds(&self) -> Vec<&str> {
        vec![
            "ps auxww",
            "ps -eo pid,ppid,user,lstart,etime,args --sort=lstart",
        ]
    }

    fn service_list_cmd(&self) -> &str {
        "systemctl list-units --type=service --all --no-pager --no-legend"
    }

    fn service_show_cmd(&self, name: &str) -> Option<String> {
        // Validate service name to prevent injection
        if !is_safe_service_name(name) {
            return None;
        }
        Some(format!("systemctl show {} --no-pager", name))
    }

    fn service_cat_cmd(&self, name: &str) -> Option<String> {
        if !is_safe_service_name(name) {
            return None;
        }
        Some(format!("systemctl cat {} 2>/dev/null", name))
    }

    fn ports_cmd(&self) -> &str {
        "ss -lntup"
    }

    fn package_cmds(&self) -> Vec<&str> {
        vec![
            "dpkg -l 2>/dev/null",
            "rpm -qa --queryformat '%{NAME} %{VERSION}-%{RELEASE} %{ARCH}\\n' 2>/dev/null",
        ]
    }

    fn scheduled_task_cmds(&self) -> Vec<&str> {
        vec![
            "systemctl list-timers --all --no-pager --no-legend",
            "cat /etc/crontab 2>/dev/null",
            "ls -la /etc/cron.d/ 2>/dev/null",
        ]
    }

    fn read_file_cmd(&self, path: &str) -> Option<String> {
        // Validate path to prevent injection
        if !is_safe_path(path) {
            return None;
        }
        // Only allow reading from specific directories
        let allowed_prefixes = ["/etc/", "/opt/", "/srv/", "/var/log/", "/home/"];
        if !allowed_prefixes.iter().any(|prefix| path.starts_with(prefix)) {
            return None;
        }
        Some(format!("cat '{}' 2>/dev/null | head -c 1048576", path)) // Max 1MB
    }

    fn journal_cmd(&self, unit: &str, since: &str) -> Option<String> {
        if !is_safe_service_name(unit) {
            return None;
        }
        Some(format!(
            "journalctl --since '{}' -u {} --no-pager | tail -n 2000",
            since, unit
        ))
    }
}

/// Windows commands using PowerShell.
pub struct WindowsCommands;

impl WindowsCommands {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsCommands {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSet for WindowsCommands {
    fn hostname_cmd(&self) -> &str {
        "hostname"
    }

    fn os_version_cmd(&self) -> Option<&str> {
        Some("(Get-CimInstance Win32_OperatingSystem).Caption")
    }

    fn kernel_version_cmd(&self) -> Option<&str> {
        Some("(Get-CimInstance Win32_OperatingSystem).Version")
    }

    fn architecture_cmd(&self) -> Option<&str> {
        Some("(Get-CimInstance Win32_OperatingSystem).OSArchitecture")
    }

    fn process_cmds(&self) -> Vec<&str> {
        vec![
            "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine,CreationDate | ConvertTo-Json -Depth 3",
        ]
    }

    fn service_list_cmd(&self) -> &str {
        "Get-CimInstance Win32_Service | Select-Object Name,State,StartMode,PathName,DisplayName,Description | ConvertTo-Json -Depth 3"
    }

    fn service_show_cmd(&self, name: &str) -> Option<String> {
        if !is_safe_service_name(name) {
            return None;
        }
        Some(format!(
            "Get-CimInstance Win32_Service -Filter \"Name='{}'\" | Select-Object * | ConvertTo-Json -Depth 3",
            name
        ))
    }

    fn service_cat_cmd(&self, _name: &str) -> Option<String> {
        None // Windows doesn't have unit files
    }

    fn ports_cmd(&self) -> &str {
        "Get-NetTCPConnection | Where-Object {$_.State -eq 'Listen'} | Select-Object LocalAddress,LocalPort,OwningProcess,State | ConvertTo-Json -Depth 3"
    }

    fn package_cmds(&self) -> Vec<&str> {
        vec![
            "Get-Package | Select-Object Name,Version | ConvertTo-Json -Depth 3",
        ]
    }

    fn scheduled_task_cmds(&self) -> Vec<&str> {
        vec![
            "Get-ScheduledTask | Select-Object TaskName,State,TaskPath | ConvertTo-Json -Depth 3",
        ]
    }

    fn read_file_cmd(&self, path: &str) -> Option<String> {
        if !is_safe_path(path) {
            return None;
        }
        // Only allow reading from specific directories
        let allowed_prefixes = ["C:\\ProgramData\\", "C:\\Program Files\\", "C:\\inetpub\\"];
        let normalized = path.replace('/', "\\");
        if !allowed_prefixes.iter().any(|prefix| normalized.starts_with(prefix)) {
            return None;
        }
        Some(format!("Get-Content -Path '{}' -TotalCount 10000 -ErrorAction SilentlyContinue", path))
    }

    fn journal_cmd(&self, _unit: &str, since: &str) -> Option<String> {
        // Windows event log for Service Control Manager
        Some(format!(
            "Get-WinEvent -FilterHashtable @{{LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddHours(-1)}} -MaxEvents 100 -ErrorAction SilentlyContinue | Select-Object TimeCreated,Message | ConvertTo-Json -Depth 3"
        ))
    }
}

/// Validate that a service name is safe (no injection).
fn is_safe_service_name(name: &str) -> bool {
    // Allow alphanumeric, dash, underscore, dot, @
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '@')
        && !name.is_empty()
        && name.len() < 256
}

/// Validate that a path is safe (no injection).
fn is_safe_path(path: &str) -> bool {
    // Disallow command injection characters
    !path.contains(';')
        && !path.contains('|')
        && !path.contains('&')
        && !path.contains('$')
        && !path.contains('`')
        && !path.contains('\n')
        && !path.contains('\r')
        && !path.contains("..") // No path traversal
        && !path.is_empty()
        && path.len() < 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_service_name() {
        assert!(is_safe_service_name("nginx"));
        assert!(is_safe_service_name("my-service"));
        assert!(is_safe_service_name("my_service"));
        assert!(is_safe_service_name("my.service"));
        assert!(is_safe_service_name("my-service@instance"));

        assert!(!is_safe_service_name("service; rm -rf /"));
        assert!(!is_safe_service_name("service | cat /etc/passwd"));
        assert!(!is_safe_service_name(""));
    }

    #[test]
    fn test_safe_path() {
        assert!(is_safe_path("/etc/nginx/nginx.conf"));
        assert!(is_safe_path("/opt/app/config.yaml"));

        assert!(!is_safe_path("/etc/passwd; rm -rf /"));
        assert!(!is_safe_path("/etc/../../../passwd"));
        assert!(!is_safe_path("/etc/passwd | cat"));
        assert!(!is_safe_path(""));
    }

    #[test]
    fn test_linux_commands() {
        let cmds = LinuxCommands::new();

        assert_eq!(cmds.hostname_cmd(), "hostname");
        assert!(cmds.os_version_cmd().is_some());
        assert!(cmds.service_show_cmd("nginx").is_some());
        assert!(cmds.service_show_cmd("nginx; rm -rf /").is_none());
    }

    #[test]
    fn test_read_file_allowed_paths() {
        let cmds = LinuxCommands::new();

        assert!(cmds.read_file_cmd("/etc/nginx/nginx.conf").is_some());
        assert!(cmds.read_file_cmd("/opt/app/config.yaml").is_some());
        assert!(cmds.read_file_cmd("/var/log/app.log").is_some());

        // Disallowed paths
        assert!(cmds.read_file_cmd("/root/.bashrc").is_none());
        assert!(cmds.read_file_cmd("/tmp/secret").is_none());
    }
}

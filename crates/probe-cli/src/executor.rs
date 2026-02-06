//! Command executors for different connection types.

use anyhow::{Context, Result};
use async_trait::async_trait;
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use tracing::{debug, warn};

/// Trait for command execution.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a command and return (exit_code, stdout, stderr).
    async fn execute(&self, command: &str) -> Result<(Option<i32>, String, String)>;

    /// Check if the connection is still alive.
    fn is_connected(&self) -> bool;
}

/// Local executor for ephemeral testing.
pub struct LocalExecutor;

impl LocalExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Executor for LocalExecutor {
    async fn execute(&self, command: &str) -> Result<(Option<i32>, String, String)> {
        debug!("Local exec: {}", command);

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .output()
                .context("Failed to execute command")?
        } else {
            Command::new("sh")
                .args(["-c", command])
                .output()
                .context("Failed to execute command")?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        Ok((exit_code, stdout, stderr))
    }

    fn is_connected(&self) -> bool {
        true
    }
}

/// SSH executor for remote Linux systems.
pub struct SshExecutor {
    session: Session,
}

impl SshExecutor {
    /// Connect to a remote host via SSH.
    pub fn connect(
        host: &str,
        port: u16,
        user: Option<&str>,
        key_path: Option<&Path>,
        password: Option<&str>,
    ) -> Result<Self> {
        let tcp = TcpStream::connect(format!("{}:{}", host, port))
            .context("Failed to connect to SSH host")?;

        let mut session = Session::new().context("Failed to create SSH session")?;
        session.set_tcp_stream(tcp);
        session.handshake().context("SSH handshake failed")?;

        let username = user.unwrap_or("root");

        // Try key-based auth first
        if let Some(key) = key_path {
            session
                .userauth_pubkey_file(username, None, key, None)
                .context("SSH key authentication failed")?;
        } else if let Some(pwd) = password {
            session
                .userauth_password(username, pwd)
                .context("SSH password authentication failed")?;
        } else {
            // Try agent
            let mut agent = session.agent().context("Failed to connect to SSH agent")?;
            agent.connect().context("Failed to connect to SSH agent")?;
            agent
                .list_identities()
                .context("Failed to list SSH agent identities")?;

            let identities = agent.identities()?;
            let mut authenticated = false;

            for identity in identities {
                if agent.userauth(username, &identity).is_ok() {
                    authenticated = true;
                    break;
                }
            }

            if !authenticated {
                anyhow::bail!("No valid SSH authentication method available");
            }
        }

        if !session.authenticated() {
            anyhow::bail!("SSH authentication failed");
        }

        Ok(Self { session })
    }
}

#[async_trait]
impl Executor for SshExecutor {
    async fn execute(&self, command: &str) -> Result<(Option<i32>, String, String)> {
        debug!("SSH exec: {}", command);

        let mut channel = self
            .session
            .channel_session()
            .context("Failed to open SSH channel")?;
        channel
            .exec(command)
            .context("Failed to execute SSH command")?;

        let mut stdout = String::new();
        channel
            .read_to_string(&mut stdout)
            .context("Failed to read stdout")?;

        let mut stderr = String::new();
        channel
            .stderr()
            .read_to_string(&mut stderr)
            .context("Failed to read stderr")?;

        channel.wait_close().ok();
        let exit_code = channel.exit_status().ok();

        Ok((exit_code, stdout, stderr))
    }

    fn is_connected(&self) -> bool {
        self.session.authenticated()
    }
}

/// WinRM executor for remote Windows systems.
pub struct WinRmExecutor {
    endpoint: String,
    username: String,
    password: String,
    client: reqwest::Client,
}

impl WinRmExecutor {
    /// Connect to a remote host via WinRM.
    pub async fn connect(
        host: &str,
        port: u16,
        https: bool,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self> {
        let scheme = if https { "https" } else { "http" };
        let endpoint = format!("{}://{}:{}/wsman", scheme, host, port);

        let username = user.unwrap_or("Administrator").to_string();
        let password = password.unwrap_or("").to_string();

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // For testing only
            .build()
            .context("Failed to create HTTP client")?;

        // Test connection
        let test_result =
            Self::execute_winrm_command(&client, &endpoint, &username, &password, "hostname").await;
        if test_result.is_err() {
            warn!("WinRM connection test failed, continuing anyway");
        }

        Ok(Self {
            endpoint,
            username,
            password,
            client,
        })
    }

    async fn execute_winrm_command(
        client: &reqwest::Client,
        endpoint: &str,
        username: &str,
        password: &str,
        command: &str,
    ) -> Result<(Option<i32>, String, String)> {
        // Encode command as UTF-16LE base64 for PowerShell
        let utf16_bytes: Vec<u8> = command
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let encoded_command = base64_encode(&utf16_bytes);

        // WinRM SOAP envelope for PowerShell command execution
        // This is a simplified implementation - real WinRM requires proper shell management
        let soap_body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd">
  <s:Header>
    <a:To>{}</a:To>
    <w:ResourceURI s:mustUnderstand="true">http://schemas.microsoft.com/wbem/wsman/1/windows/shell/cmd</w:ResourceURI>
    <a:Action s:mustUnderstand="true">http://schemas.microsoft.com/wbem/wsman/1/windows/shell/Command</a:Action>
  </s:Header>
  <s:Body>
    <rsp:CommandLine xmlns:rsp="http://schemas.microsoft.com/wbem/wsman/1/windows/shell">
      <rsp:Command>powershell.exe</rsp:Command>
      <rsp:Arguments>-NoProfile -NonInteractive -EncodedCommand {}</rsp:Arguments>
    </rsp:CommandLine>
  </s:Body>
</s:Envelope>"#,
            endpoint, encoded_command
        );

        let response = client
            .post(endpoint)
            .basic_auth(username, Some(password))
            .header("Content-Type", "application/soap+xml;charset=UTF-8")
            .body(soap_body)
            .send()
            .await
            .context("WinRM request failed")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read WinRM response")?;

        if status.is_success() {
            // Parse SOAP response to extract output
            // This is a simplified implementation
            Ok((Some(0), body, String::new()))
        } else {
            Ok((
                Some(1),
                String::new(),
                format!("WinRM error: {} - {}", status, body),
            ))
        }
    }
}

#[async_trait]
impl Executor for WinRmExecutor {
    async fn execute(&self, command: &str) -> Result<(Option<i32>, String, String)> {
        debug!("WinRM exec: {}", command);
        Self::execute_winrm_command(
            &self.client,
            &self.endpoint,
            &self.username,
            &self.password,
            command,
        )
        .await
    }

    fn is_connected(&self) -> bool {
        true // WinRM is stateless
    }
}

/// Simple base64 encoding (to avoid adding another dependency).
fn base64_encode(data: &[u8]) -> String {
    // Just use a simple base64 implementation
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let mut i = 0;

    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as usize
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as usize
        } else {
            0
        };

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if i + 1 < data.len() {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if i + 2 < data.len() {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }

        i += 3;
    }

    result
}

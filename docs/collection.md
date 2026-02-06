# Collection Guide

## Overview

The `probe-cli collect` command gathers system information from a target host.

## Usage

```bash
probe-cli collect \
  --target <host> \
  --os <linux|windows> \
  --mode <remote|local-ephemeral> \
  --out <bundle.tgz>
```

## Connection Options

### Linux (SSH)

```bash
probe-cli collect \
  --target 192.168.1.100 \
  --os linux \
  --ssh-user admin \
  --ssh-key ~/.ssh/id_rsa \
  --out bundle.tgz
```

Options:
- `--ssh-port`: SSH port (default: 22)
- `--ssh-user`: SSH username
- `--ssh-key`: Path to SSH private key
- `--ssh-password`: SSH password (not recommended)

### Windows (WinRM)

```bash
probe-cli collect \
  --target 192.168.1.100 \
  --os windows \
  --winrm-user Administrator \
  --winrm-password 'password' \
  --out bundle.tgz
```

Options:
- `--winrm-port`: WinRM port (default: 5985)
- `--winrm-https`: Use HTTPS for WinRM

### Local Testing

```bash
probe-cli collect \
  --target localhost \
  --os linux \
  --mode local-ephemeral \
  --out bundle.tgz
```

## Commands Executed

### Linux

| Category | Commands |
|----------|----------|
| Processes | `ps auxww`, `ps -eo pid,ppid,user,lstart,etime,args` |
| Services | `systemctl list-units`, `systemctl show <unit>`, `systemctl cat <unit>` |
| Ports | `ss -lntup` |
| Packages | `dpkg -l` or `rpm -qa` |
| Logs | `journalctl --since "<t0>" -u <unit>` |

### Windows

| Category | Commands |
|----------|----------|
| Processes | `Get-CimInstance Win32_Process` |
| Services | `Get-CimInstance Win32_Service` |
| Ports | `Get-NetTCPConnection` |
| Tasks | `Get-ScheduledTask` |
| Events | `Get-WinEvent` |

## File Collection

Files are collected only from:
- Paths referenced in service configurations
- Standard configuration directories (`/etc`, `/opt`, `/srv`)
- Working directories of discovered services

Files are limited to 1MB and redacted before storage.

## Security

### Allowlist

Only pre-defined commands can be executed. The allowlist prevents:
- Command injection via service names or paths
- Path traversal attacks
- Execution of arbitrary code

### Redaction

Before writing to the bundle, all content is scanned for:
- Password patterns
- API keys and tokens
- Connection strings
- High-entropy strings (likely secrets)

## Bundle Validation

After collection, validate the bundle:

```bash
probe-cli validate-bundle --in bundle.tgz --check-evidence --verify-checksums
```

This verifies:
- JSON schema compliance
- Evidence file existence
- Checksum integrity

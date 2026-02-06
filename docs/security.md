# Security Guide

## Overview

XCProbe is designed with security as a primary concern.

## Non-Intrusive Collection

### Read-Only Operations

All collection commands are read-only:
- No files are written on the target
- No processes are modified
- No configuration is changed

### Allowlisted Commands

Only pre-approved commands can be executed:

```rust
// Example: service name validation
fn is_safe_service_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !name.is_empty()
        && name.len() < 256
}
```

This prevents:
- Command injection via service names
- Shell metacharacter injection
- Path traversal attacks

### Path Restrictions

File reading is limited to:
- `/etc/` - Configuration files
- `/opt/` - Application files
- `/srv/` - Service data
- `/var/log/` - Log files
- `/home/<user>/` - User directories

## Secret Redaction

### Pattern-Based Detection

Secrets are detected using regex patterns:

| Pattern | Examples |
|---------|----------|
| Password keywords | `password=`, `passwd:`, `pwd=` |
| Token keywords | `token=`, `api_key=`, `secret=` |
| Authorization headers | `Authorization: Bearer ...` |
| Connection strings | `postgres://user:pass@...` |
| AWS keys | `AKIA...`, `aws_secret_access_key` |

### Entropy-Based Detection

High-entropy strings are flagged as potential secrets:

```rust
fn is_high_entropy(s: &str, threshold: f64) -> bool {
    if s.len() < 16 || s.len() > 256 {
        return false;
    }
    shannon_entropy(s) >= threshold // default: 4.0
}
```

### Redaction Modes

1. **Standard**: Replace with `[REDACTED]`
2. **Hash mode**: Replace with `[HASH:abc123...]` (allows correlation)

## Audit Trail

### audit.jsonl

Every command is logged:

```json
{
  "seq": 1,
  "started_at": "2024-01-01T00:00:00Z",
  "completed_at": "2024-01-01T00:00:01Z",
  "command": "ps auxww",
  "exit_code": 0,
  "evidence_ref": "evidence/ps_001.txt"
}
```

### Checksums

Bundle integrity is verified:

```json
{
  "manifest.json": "sha256:abc...",
  "evidence/ps_001.txt": "sha256:def..."
}
```

## Recommendations

### Collection Security

1. **Use SSH keys**: Avoid password authentication
2. **Limit permissions**: Use a dedicated collection user with minimal rights
3. **Network isolation**: Collect from a management network
4. **Secure storage**: Protect bundles (they contain system information)

### Analysis Security

1. **Review artifacts**: Generated Dockerfiles should be reviewed
2. **No secrets in images**: Verify no sensitive data leaked
3. **Template variables**: Use environment variables for all secrets

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Command injection | Allowlist + input validation |
| Data exfiltration | Redaction before storage |
| Privilege escalation | Read-only operations |
| Information disclosure | Bundle encryption (planned) |

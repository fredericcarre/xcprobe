# XCProbe

**Non-intrusive system discovery and containerization tool.**

XCProbe analyses a running Linux or Windows host, identifies its business applications (web servers, APIs, workers, databases, etc.), and generates ready-to-use Docker artifacts (Dockerfile, docker-compose.yaml, entrypoint scripts) so you can containerize them.

The whole process is **read-only** and **agentless**: nothing is installed on the target, no kernel tracing is performed, and sensitive data is automatically redacted.

## How it works

```
  Target Host                    Your Workstation
 ┌──────────────┐               ┌──────────────────────────────────────┐
 │  Linux/Win   │   SSH/WinRM   │                                      │
 │  server      │◄──────────────│  xcprobe collect  ──► bundle.tgz     │
 │              │  (read-only)  │                                      │
 └──────────────┘               │  xcprobe analyze  ──► ./artifacts/   │
                                │       ├── Dockerfile                 │
                                │       ├── docker-compose.yaml        │
                                │       ├── entrypoint.sh              │
                                │       ├── packplan.json              │
                                │       └── README.md                  │
                                └──────────────────────────────────────┘
```

1. **Collect** gathers system information (processes, services, ports, configs, env vars, logs) from the target host via SSH or WinRM.
2. **Analyze** processes the collected bundle, scores processes for business relevance, clusters them into logical applications, detects dependencies, and generates Docker artifacts.

## Download

Pre-built binaries are available on the [GitHub Releases](https://github.com/fredericcarre/xcprobe/releases) page for:

| Platform | Archive |
|----------|---------|
| Linux x86_64 (glibc) | `xcprobe-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 (musl/static) | `xcprobe-vX.Y.Z-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `xcprobe-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `xcprobe-vX.Y.Z-x86_64-pc-windows-msvc.zip` |

```bash
# Example: download and install on Linux
curl -LO https://github.com/fredericcarre/xcprobe/releases/latest/download/xcprobe-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf xcprobe-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
chmod +x xcprobe
sudo mv xcprobe /usr/local/bin/
```

## Usage

### 1. Collect from a remote Linux host (SSH)

```bash
xcprobe collect \
  --target 192.168.1.100 \
  --os linux \
  --ssh-user admin \
  --ssh-key ~/.ssh/id_rsa \
  --out bundle.tgz
```

You can also use password authentication:

```bash
xcprobe collect \
  --target 192.168.1.100 \
  --os linux \
  --ssh-user admin \
  --ssh-password "s3cret" \
  --out bundle.tgz
```

### 2. Collect from a remote Windows host (WinRM)

```bash
xcprobe collect \
  --target 192.168.1.200 \
  --os windows \
  --winrm-user Administrator \
  --winrm-password "s3cret" \
  --out bundle.tgz

# With HTTPS
xcprobe collect \
  --target 192.168.1.200 \
  --os windows \
  --winrm-user Administrator \
  --winrm-password "s3cret" \
  --winrm-https \
  --out bundle.tgz
```

### 3. Collect from localhost (testing / development)

```bash
xcprobe collect \
  --target localhost \
  --os linux \
  --mode local-ephemeral \
  --out bundle.tgz
```

### 4. Analyze a bundle and generate Docker artifacts

```bash
xcprobe analyze \
  --bundle bundle.tgz \
  --out ./artifacts
```

With custom options:

```bash
xcprobe analyze \
  --bundle bundle.tgz \
  --out ./artifacts \
  --cluster-prefix myapp \
  --min-confidence 0.5
```

The generated `./artifacts/` directory will contain:
- **Dockerfile** per detected application
- **docker-compose.yaml** orchestrating all applications
- **entrypoint.sh** startup scripts
- **packplan.json** full analysis plan with evidence and confidence scores
- **README.md** documentation for the generated artifacts

## CLI Reference

```
xcprobe [OPTIONS] <COMMAND>

Options:
  -v, --verbose    Enable verbose output
  -h, --help       Print help
  -V, --version    Print version

Commands:
  collect    Collect system information from a target host
  analyze    Analyze a bundle and generate Docker artifacts
```

### `xcprobe collect`

| Option | Description | Default |
|--------|-------------|---------|
| `--target <HOST>` | Target host (IP, hostname, or `localhost`) | *required* |
| `--os <OS>` | Target OS: `linux` or `windows` | *required* |
| `-o, --out <PATH>` | Output bundle path (`.tgz`) | *required* |
| `--mode <MODE>` | `remote` or `local-ephemeral` | `remote` |
| `--timeout <SECS>` | Collection timeout | `300` |
| `--ssh-user <USER>` | SSH username | |
| `--ssh-key <PATH>` | SSH private key | |
| `--ssh-password <PASS>` | SSH password | |
| `--ssh-port <PORT>` | SSH port | `22` |
| `--winrm-user <USER>` | WinRM username | |
| `--winrm-password <PASS>` | WinRM password | |
| `--winrm-port <PORT>` | WinRM port | `5985` |
| `--winrm-https` | Use HTTPS for WinRM | `false` |

### `xcprobe analyze`

| Option | Description | Default |
|--------|-------------|---------|
| `--bundle <PATH>` | Input bundle (`.tgz`) | *required* |
| `-o, --out <DIR>` | Output directory for artifacts | *required* |
| `--cluster-prefix <STR>` | Prefix for cluster IDs | `app` |
| `--min-confidence <FLOAT>` | Minimum confidence threshold (0.0-1.0) | `0.7` |

## What gets collected

| Data | Linux | Windows |
|------|-------|---------|
| System info | hostname, OS, kernel | hostname, OS, architecture |
| Processes | `ps auxww` | `Get-CimInstance Win32_Process` |
| Services | `systemctl` units | `Get-CimInstance Win32_Service` |
| Network ports | `ss -lntup` | `Get-NetTCPConnection` |
| Packages | `dpkg -l`, `rpm -qa` | `Get-Package` |
| Scheduled tasks | systemd timers, crontabs | `Get-ScheduledTask` |
| Logs | `journalctl` (last 2000 lines) | Event logs (last 100 events) |
| Config files | `/etc/`, `/opt/`, `/srv/` (max 1 MB each) | `C:\ProgramData\`, `C:\Program Files\` |

## Non-intrusive guarantees

| Aspect | Guarantee |
|--------|-----------|
| Agent | No persistent agent installed |
| Kernel | No eBPF, strace, or auditd |
| Access | Read-only operations only |
| Commands | Allowlisted commands only |
| Secrets | Redacted before writing to bundle |
| Analysis | Performed offline after collection |

## Development

### Build from source

```bash
cargo build --release
# Binary: target/release/xcprobe
```

### Run E2E tests

E2E tests require Docker and use simulated host containers.

```bash
# Run all scenarios
cargo run --bin e2e-runner -- run-all --scenarios-dir tests/scenarios

# Run a single scenario
cargo run --bin e2e-runner -- run --scenario tests/scenarios/scenario_a_basic_multi_proc_host
```

### Project structure

```
xcprobe/
├── crates/
│   ├── xcprobe/          # CLI binary (collect + analyze)
│   ├── probe-cli/        # Collection logic (library)
│   ├── analyzer/         # Analysis, scoring, clustering, Docker generation (library)
│   ├── bundle-schema/    # Bundle and pack-plan format definitions
│   ├── redaction/        # Secret detection and masking
│   ├── e2e-runner/       # E2E test runner (dev only)
│   └── common/           # Shared utilities
├── tests/scenarios/      # E2E test scenarios
├── docs/                 # Documentation
└── .github/workflows/    # CI and release pipelines
```

## License

MIT

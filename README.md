# XCProbe

**Non-intrusive system discovery and containerization tool.**

XCProbe collects information from Linux and Windows systems without installing persistent agents, analyzes the collected data to identify business applications, and generates Docker artifacts for containerization.

## Features

- **Non-intrusive collection**: Read-only operations, no kernel tracing, no agents
- **Evidence-based analysis**: Every decision is backed by collected evidence
- **Secret redaction**: Automatic detection and masking of sensitive data
- **Docker artifact generation**: Dockerfile, entrypoint, compose files
- **E2E testing framework**: Validate detection accuracy with CI

## Quick Start

### Build

```bash
cargo build --release
```

### Collect from a Linux host

```bash
# Via SSH
probe-cli collect --target 192.168.1.100 --os linux \
  --ssh-user admin --ssh-key ~/.ssh/id_rsa \
  --out bundle.tgz

# Local (for testing)
probe-cli collect --target localhost --os linux \
  --mode local-ephemeral --out bundle.tgz
```

### Analyze and generate Docker artifacts

```bash
analyzer analyze --bundle bundle.tgz --out ./artifacts
```

### Run E2E tests

```bash
# Run a single scenario
e2e-runner run --scenario tests/scenarios/scenario_a_basic_multi_proc_host

# Run all scenarios
e2e-runner run-all --scenarios-dir tests/scenarios
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          XCProbe                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │  probe-cli   │───▶│    Bundle    │───▶│   analyzer   │      │
│  │  (collect)   │    │   (tgz)      │    │  (analyze)   │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│         │                                       │               │
│         ▼                                       ▼               │
│  ┌──────────────┐                      ┌──────────────┐        │
│  │ Target Host  │                      │   Artifacts  │        │
│  │ (SSH/WinRM)  │                      │  Dockerfile  │        │
│  └──────────────┘                      │  compose.yaml│        │
│                                        │  README.md   │        │
│                                        └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

## Documentation

- [Collection](docs/collection.md) - How to collect system information
- [Analysis](docs/analysis.md) - Understanding the analysis pipeline
- [Docker Generation](docs/docker-generation.md) - Generated artifact details
- [Security](docs/security.md) - Redaction and security considerations
- [Workflow](docs/workflow.md) - End-to-end migration workflow
- [E2E CI](docs/e2e-ci.md) - Writing and running E2E tests

## Non-Intrusive Guarantees

XCProbe is designed to be non-intrusive:

| Aspect | Guarantee |
|--------|-----------|
| Agent | ❌ No persistent agent installed |
| Kernel | ❌ No eBPF, strace, auditd |
| Access | ✅ Read-only operations only |
| Commands | ✅ Allowlisted commands only |
| Secrets | ✅ Redacted before writing |
| Analysis | ✅ Offline after collection |

## Project Structure

```
xcprobe/
├── crates/
│   ├── probe-cli/       # Collection tool
│   ├── analyzer/        # Analysis & Docker generation
│   ├── bundle-schema/   # Bundle format definitions
│   ├── redaction/       # Secret masking
│   ├── e2e-runner/      # E2E test runner
│   └── common/          # Shared utilities
├── docs/                # Documentation
├── tests/scenarios/     # E2E test scenarios
└── .github/workflows/   # CI configuration
```

## License

MIT

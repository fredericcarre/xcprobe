# XCProbe Overview

XCProbe is a system discovery and containerization tool designed for lift-and-shift migrations. It analyzes running systems to understand their applications and generates Docker artifacts.

## Design Principles

### Non-Intrusive

XCProbe never installs persistent agents or modifies the target system:

- **Read-only**: All operations are read-only
- **No kernel hooks**: No eBPF, strace, auditd, or similar
- **Allowlisted commands**: Only pre-approved commands are executed
- **Offline analysis**: Analysis happens after collection, not on the target

### Evidence-Based

Every decision made by XCProbe is backed by evidence:

- Each decision includes `evidence_refs` pointing to collected data
- Confidence scores reflect the quality of evidence
- No "hallucinated" assumptions

### Security-First

- Secrets are automatically redacted before writing
- High-entropy strings are detected and masked
- Connection strings, tokens, and passwords are never stored

## Components

### probe-cli

The collection tool that connects to target systems and gathers information:

- Process listings
- Service configurations (systemd, Windows services)
- Network ports and connections
- Installed packages
- Configuration files
- Log snippets

### analyzer

The analysis engine that processes bundles and generates artifacts:

- Scores processes for business relevance
- Clusters related processes into applications
- Detects dependencies between applications
- Generates Dockerfiles, entrypoints, and compose files

### e2e-runner

The testing framework for validating detection accuracy:

- Runs test scenarios with docker-compose
- Compares results to ground truth
- Calculates precision/recall metrics

## Bundle Format

Collection results are packaged as a compressed tarball:

```
bundle.tgz
├── manifest.json       # All collected facts
├── audit.jsonl         # Command execution log
├── evidence/           # Raw command outputs
├── attachments/        # Config/log snippets (redacted)
└── checksums.json      # Integrity verification
```

## Pack Plan Format

Analysis results are output as JSON:

```json
{
  "clusters": [
    {
      "id": "app-0",
      "name": "api",
      "app_type": "api",
      "ports": [{"port": 8080}],
      "decisions": [
        {
          "decision": "Service listens on port 8080",
          "evidence_refs": ["evidence/ss_lntup.txt"]
        }
      ]
    }
  ]
}
```

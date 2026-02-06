# E2E Testing Guide

## Overview

XCProbe includes an end-to-end testing framework for validating detection accuracy.

## Test Scenario Structure

```
tests/scenarios/scenario_name/
├── compose.yaml          # Docker Compose for test environment
├── truth.json            # Expected detection results
└── host-sim/             # Simulated host configuration
    ├── Dockerfile
    └── ...
```

## truth.json Format

```json
{
  "version": "1.0",
  "name": "scenario_name",
  "description": "Description of the test scenario",
  "applications": [
    {
      "name": "api",
      "app_type": "api",
      "command_patterns": ["python", "flask"],
      "user": "api-user",
      "working_directory": "/opt/api"
    }
  ],
  "ports": [
    {"port": 8080, "protocol": "tcp", "process_name": "python"}
  ],
  "env_names": [
    "DATABASE_URL",
    "REDIS_URL"
  ],
  "dependencies": [
    {"from": "api", "to": "db", "dep_type": "database"}
  ],
  "config_files": [
    "/etc/app/config.yaml"
  ],
  "thresholds": {
    "process_cmdline_recall": 0.9,
    "ports_recall": 0.95,
    "env_names_recall": 0.8,
    "deps_recall": 0.8,
    "require_all_evidence": true
  }
}
```

## Running Tests

### Single Scenario

```bash
e2e-runner run \
  --scenario tests/scenarios/scenario_a_basic_multi_proc_host \
  --artifacts ./artifacts
```

### All Scenarios

```bash
e2e-runner run-all \
  --scenarios-dir tests/scenarios \
  --artifacts ./artifacts \
  --continue-on-failure
```

## Metrics

### Recall

How many expected items were detected:

```
recall = |detected ∩ expected| / |expected|
```

### Precision

How many detected items were correct:

```
precision = |detected ∩ expected| / |detected|
```

### Thresholds

| Metric | Default | Description |
|--------|---------|-------------|
| process_cmdline_recall | 0.9 | Command line detection |
| ports_recall | 0.95 | Port detection |
| env_names_recall | 0.8 | Environment variable detection |
| deps_recall | 0.8 | Dependency detection |
| require_all_evidence | true | All decisions need evidence |

## Test Report

### Text Format

```
=== XCProbe E2E Test Report ===

Summary:
  Total:  4
  Passed: 3 (75.0%)
  Failed: 1 (25.0%)

Scenario Results:
--------------------------------------------------------------------------------
Scenario                            Status       Proc%      Port%     Time(s)
--------------------------------------------------------------------------------
scenario_a_basic_multi_proc_host      PASS       95.0       100.0       12.34
scenario_b_wrapper_and_env_files      PASS       90.0       100.0       10.21
scenario_c_batch_only_no_ports        FAIL       80.0       100.0        8.56
scenario_d_logs_stdout_only           PASS       85.0        95.0        9.12
--------------------------------------------------------------------------------
```

### JSON Format

```bash
e2e-runner report --results ./artifacts --format json
```

## Creating a New Scenario

### 1. Create Directory Structure

```bash
mkdir -p tests/scenarios/my_scenario/host-sim
```

### 2. Create compose.yaml

```yaml
services:
  host-sim:
    build: ./host-sim
    hostname: test-host

  # Add external dependencies as needed
  db:
    image: postgres:15-alpine
```

### 3. Create host-sim/Dockerfile

```dockerfile
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y python3 supervisor

# Copy application files
COPY apps/ /opt/
COPY supervisord.conf /etc/supervisor/supervisord.conf

CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/supervisord.conf"]
```

### 4. Create truth.json

Define what should be detected:
- Applications (name, type, command patterns)
- Ports
- Environment variables
- Dependencies
- Config files
- Pass/fail thresholds

### 5. Test Locally

```bash
# Start the environment
cd tests/scenarios/my_scenario
docker compose up -d

# Run collection manually
probe-cli collect --target localhost --os linux --mode local-ephemeral --out test.tgz

# Analyze
analyzer analyze --bundle test.tgz --out ./test-artifacts

# Compare to truth
# (manually verify or use e2e-runner)

# Cleanup
docker compose down -v
```

## CI Integration

Add to `.github/workflows/ci.yml`:

```yaml
- name: Run E2E Tests
  run: |
    e2e-runner run-all \
      --scenarios-dir tests/scenarios \
      --artifacts ./artifacts

- name: Upload Failed Artifacts
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: e2e-failed-artifacts
    path: artifacts/*/failed_artifacts/
```

## Troubleshooting

### Test Fails with Low Recall

1. Check if probe-cli is finding the processes/services
2. Verify host-sim is running correctly
3. Check bundle contents manually

### Evidence Missing

1. Verify commands are succeeding in host-sim
2. Check for permission issues
3. Review audit.jsonl for errors

### Compose Fails to Start

1. Check for port conflicts
2. Verify images are available
3. Check host-sim build logs

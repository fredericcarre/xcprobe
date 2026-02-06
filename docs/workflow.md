# Migration Workflow

## Overview

XCProbe supports a systematic migration workflow from discovery to containerization.

## Workflow Stages

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   1. COLLECT      2. ANALYZE      3. REVIEW      4. ITERATE    │
│   ┌─────────┐     ┌─────────┐     ┌─────────┐    ┌─────────┐   │
│   │ Target  │────▶│ Bundle  │────▶│Artifacts│───▶│ Refine  │   │
│   │ System  │     │         │     │         │    │ & Test  │   │
│   └─────────┘     └─────────┘     └─────────┘    └─────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Stage 1: Collection

### Prepare

1. Ensure SSH/WinRM access to target
2. Verify network connectivity
3. Schedule collection during normal operation

### Collect

```bash
probe-cli collect \
  --target prod-server-01 \
  --os linux \
  --ssh-user collector \
  --ssh-key ~/.ssh/collector_key \
  --out prod-server-01.tgz
```

### Validate

```bash
probe-cli validate-bundle \
  --in prod-server-01.tgz \
  --check-evidence \
  --verify-checksums
```

## Stage 2: Analysis

### Analyze

```bash
analyzer analyze \
  --bundle prod-server-01.tgz \
  --out ./migration/prod-server-01 \
  --min-confidence 0.7
```

### Review Confidence

```bash
# Check for decisions without evidence
analyzer validate-plan \
  --plan ./migration/prod-server-01/packplan.json
```

## Stage 3: Review

### Examine Generated Artifacts

For each cluster:

1. **Dockerfile**
   - Verify base image
   - Add application files
   - Install dependencies

2. **entrypoint.sh**
   - Verify template rendering
   - Adjust dependency waits

3. **README.md**
   - Review detected configuration
   - Note required environment variables

4. **confidence.json**
   - Check for low-confidence decisions
   - Address missing evidence

### Manual Additions

Common additions needed:

```dockerfile
# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libpq5 \
    ca-certificates

# Copy application files
COPY pack/ /app/

# Set proper permissions
RUN chown -R appuser:appuser /app
```

## Stage 4: Pack (Optional)

If needed, collect application files from the source:

```bash
# Generate pack plan
probe-cli plan-pack \
  --bundle prod-server-01.tgz \
  --out packplan.json

# Execute pack
probe-cli pack \
  --target prod-server-01 \
  --os linux \
  --plan packplan.json \
  --out ./pack
```

## Stage 5: Build & Test

### Local Build

```bash
cd ./migration/prod-server-01/app-0
docker build -t myapp:test .
```

### Test

```bash
docker run --rm -it \
  -e DATABASE_URL=postgres://... \
  -p 8080:8080 \
  myapp:test
```

### Integration Test

```bash
docker compose up -d
# Run integration tests
./run-tests.sh
docker compose down
```

## Stage 6: Iterate

Based on testing:

1. Adjust Dockerfiles
2. Fix configuration templates
3. Update environment variables
4. Re-test

## Best Practices

### Collection

- Collect during normal operation (not maintenance)
- Collect multiple times to catch variations
- Document any manual intervention needed

### Analysis

- Review all low-confidence decisions
- Verify dependency detection is accurate
- Cross-check against known architecture

### Review

- Have application owners review artifacts
- Document any manual changes made
- Version control all artifacts

### Testing

- Test in isolated environment first
- Verify all functionality works
- Compare behavior to source system

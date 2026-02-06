# Analysis Guide

## Overview

The `analyzer analyze` command processes a collection bundle and generates Docker artifacts.

## Usage

```bash
analyzer analyze \
  --bundle bundle.tgz \
  --out ./artifacts \
  --min-confidence 0.7
```

## Analysis Pipeline

### 1. Bundle Validation

The bundle is validated against the JSON schema:
- Required fields present
- Valid data types
- Evidence references exist

### 2. Process Scoring

Each process is scored for business relevance:

| Factor | Score Impact |
|--------|--------------|
| System process (kworker, etc.) | -0.4 |
| Known framework (node, python, java) | +0.3 |
| Listening on a port | +0.2 |
| Managed by systemd | +0.3 |
| Non-root user | +0.1 |

Processes with score >= 0.6 are considered business applications.

### 3. Application Clustering

Related processes and services are grouped:
- Services with matching PIDs
- Processes sharing working directories
- Services with common environment files

### 4. Dependency Detection

Dependencies are detected by scanning:
- Configuration files for connection strings
- Environment variables for host references
- Logs for endpoint patterns

Detection patterns:
- Database URLs: `postgres://`, `mysql://`, `mongodb://`
- Cache URLs: `redis://`, `memcached://`
- API endpoints: `http://`, `https://`

### 5. DAG Building

A startup DAG is built based on dependencies:
- Database dependencies start first
- Caches and queues next
- Application services last

### 6. Confidence Scoring

Each cluster receives a confidence score:

```
confidence = Σ(decision_confidence × weight) / Σ(weight)
```

Where weight = 1.0 for decisions with evidence, 0.5 otherwise.

## Output Structure

```
artifacts/
├── app-0/
│   ├── Dockerfile
│   ├── entrypoint.sh
│   ├── templates/
│   │   └── config.yaml.tmpl
│   ├── README.md
│   └── confidence.json
├── app-1/
│   └── ...
├── docker-compose.yaml
└── packplan.json
```

## Confidence Report

Each cluster includes `confidence.json`:

```json
{
  "cluster_id": "app-0",
  "overall_confidence": 0.85,
  "decisions": [
    {
      "decision": "Service listens on port 8080",
      "confidence": 0.95,
      "has_evidence": true,
      "evidence_refs": ["evidence/ss_lntup.txt"]
    }
  ],
  "warnings": [],
  "missing_evidence": []
}
```

## Validation

Validate that all decisions have evidence:

```bash
analyzer validate-plan --plan packplan.json --strict
```

The `--strict` flag fails if any decision lacks evidence.

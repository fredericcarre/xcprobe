# Docker Generation Guide

## Overview

XCProbe generates Docker artifacts for each discovered application cluster.

## Generated Artifacts

### Dockerfile

```dockerfile
FROM debian:bookworm-slim

WORKDIR /app
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

COPY templates/ /templates/

# Create application user
RUN adduser --disabled-password --gecos '' appuser
USER appuser

EXPOSE 8080

ENTRYPOINT ["/entrypoint.sh"]
CMD ["/usr/bin/python3", "/app/server.py"]
```

Base image is selected based on detected application type:
- Node.js → `node:20-alpine`
- Python → `python:3.11-slim`
- Java → `eclipse-temurin:17-jre-alpine`
- .NET → `mcr.microsoft.com/dotnet/aspnet:8.0`
- Default → `debian:bookworm-slim`

### entrypoint.sh

```bash
#!/bin/bash
set -e

# Render configuration templates
render_template() {
  local src="$1"
  local dst="$2"
  envsubst < "$src" > "$dst"
}

render_template /templates/config.yaml.tmpl /app/config.yaml

# Wait for dependencies
wait_for_port db 5432
wait_for_port redis 6379

# Execute the main command
exec "$@"
```

### Config Templates

Configuration files are converted to templates with environment variable placeholders:

```yaml
# config.yaml.tmpl
database:
  host: ${DATABASE_HOST}
  port: ${DATABASE_PORT}
  user: ${DATABASE_USER}
  password: ${DATABASE_PASSWORD}
```

### docker-compose.yaml

```yaml
services:
  api:
    build:
      context: ./app-0
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: "${DATABASE_URL:-}"
    depends_on:
      db:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "nc", "-z", "localhost", "8080"]
      interval: 10s
      timeout: 5s
      retries: 3
```

## Security Rules

### No Secrets

Generated artifacts never contain secrets:
- Environment variable values are placeholders
- Connection strings use variable references
- Sensitive config values are templated

### User Creation

If the source service runs as a non-root user, the Dockerfile creates a matching user.

## Customization

Generated artifacts are starting points. Review and adjust:

1. **Base image**: Verify the detected base image is appropriate
2. **Application files**: Add COPY instructions for your application
3. **Dependencies**: Install required system packages
4. **Health checks**: Adjust endpoints and intervals
5. **Templates**: Fill in actual configuration structure

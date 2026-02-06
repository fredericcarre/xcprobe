//! Docker artifact generation.

use anyhow::Result;
use xcprobe_bundle_schema::{AppCluster, ConfigFileSpec, PackPlan};

/// Generate Dockerfile for a cluster.
pub fn generate_dockerfile(cluster: &AppCluster) -> Result<String> {
    let mut dockerfile = String::new();

    // Determine base image based on app type
    let base_image = match cluster.app_type.as_str() {
        "api" | "web" => {
            // Try to detect language
            if cluster.services.iter().any(|s| {
                s.exec_start
                    .as_ref()
                    .map(|e| e.contains("node") || e.contains("npm"))
                    .unwrap_or(false)
            }) {
                "node:20-alpine"
            } else if cluster.services.iter().any(|s| {
                s.exec_start
                    .as_ref()
                    .map(|e| e.contains("python"))
                    .unwrap_or(false)
            }) {
                "python:3.11-slim"
            } else if cluster.services.iter().any(|s| {
                s.exec_start
                    .as_ref()
                    .map(|e| e.contains("java"))
                    .unwrap_or(false)
            }) {
                "eclipse-temurin:17-jre-alpine"
            } else if cluster.services.iter().any(|s| {
                s.exec_start
                    .as_ref()
                    .map(|e| e.contains("dotnet"))
                    .unwrap_or(false)
            }) {
                "mcr.microsoft.com/dotnet/aspnet:8.0"
            } else {
                "debian:bookworm-slim"
            }
        }
        "proxy" => "nginx:alpine",
        "worker" | "batch" => "debian:bookworm-slim",
        _ => "debian:bookworm-slim",
    };

    dockerfile.push_str(&format!(
        "# Auto-generated Dockerfile for {}\n",
        cluster.name
    ));
    dockerfile.push_str(&format!("# Confidence: {:.2}\n", cluster.confidence));
    dockerfile.push_str("#\n");
    dockerfile.push_str("# IMPORTANT: Review and adjust before production use.\n");
    dockerfile.push_str("# This is a lift-and-shift migration starting point.\n\n");

    dockerfile.push_str(&format!("FROM {}\n\n", base_image));

    // Add labels
    dockerfile.push_str("LABEL maintainer=\"xcprobe-generated\"\n");
    dockerfile.push_str(&format!("LABEL app.type=\"{}\"\n\n", cluster.app_type));

    // Add working directory
    let workdir = cluster
        .services
        .first()
        .and_then(|s| s.working_directory.clone())
        .unwrap_or_else(|| "/app".to_string());

    dockerfile.push_str(&format!("WORKDIR {}\n\n", workdir));

    // Copy entrypoint
    dockerfile.push_str("# Copy entrypoint script\n");
    dockerfile.push_str("COPY entrypoint.sh /entrypoint.sh\n");
    dockerfile.push_str("RUN chmod +x /entrypoint.sh\n\n");

    // Copy config templates
    if !cluster.config_files.is_empty() {
        dockerfile.push_str("# Copy configuration templates\n");
        dockerfile.push_str("COPY templates/ /templates/\n\n");
    }

    // Copy application files placeholder
    dockerfile.push_str("# Copy application files (adjust path as needed)\n");
    dockerfile.push_str("# COPY pack/ /app/\n\n");

    // Create user if service runs as non-root
    if let Some(user) = cluster.services.first().and_then(|s| s.user.clone()) {
        if user != "root" {
            dockerfile.push_str("# Create application user\n");
            dockerfile.push_str(&format!(
                "RUN adduser --disabled-password --gecos '' {} || true\n",
                user
            ));
            dockerfile.push_str(&format!("USER {}\n\n", user));
        }
    }

    // Expose ports
    if !cluster.ports.is_empty() {
        dockerfile.push_str("# Expose ports\n");
        for port in &cluster.ports {
            dockerfile.push_str(&format!("EXPOSE {}\n", port.port));
        }
        dockerfile.push('\n');
    }

    // Environment variables (placeholders only)
    if !cluster.env_vars.is_empty() {
        dockerfile.push_str("# Environment variables (set at runtime)\n");
        for env_var in &cluster.env_vars {
            if !env_var.sensitive {
                if let Some(ref default) = env_var.default_value {
                    dockerfile.push_str(&format!("ENV {}=\"{}\"\n", env_var.name, default));
                }
            } else {
                dockerfile.push_str(&format!(
                    "# ENV {} - sensitive, set at runtime\n",
                    env_var.name
                ));
            }
        }
        dockerfile.push('\n');
    }

    // Healthcheck
    if let Some(ref readiness) = cluster.readiness {
        dockerfile.push_str(&format!(
            "HEALTHCHECK --interval={}s --timeout={}s --retries={} \\\n",
            readiness.interval_seconds, readiness.timeout_seconds, readiness.retries
        ));
        match readiness.check_type.as_str() {
            "http" => {
                let path = readiness.path.as_deref().unwrap_or("/health");
                let port = readiness.port.unwrap_or(80);
                dockerfile.push_str(&format!(
                    "  CMD curl -f http://localhost:{}{} || exit 1\n\n",
                    port, path
                ));
            }
            "tcp" => {
                let port = readiness.port.unwrap_or(80);
                dockerfile.push_str(&format!("  CMD nc -z localhost {} || exit 1\n\n", port));
            }
            _ => {
                dockerfile.push_str("  CMD exit 0\n\n");
            }
        }
    }

    // Entrypoint
    dockerfile.push_str("ENTRYPOINT [\"/entrypoint.sh\"]\n");

    // Default command from service
    if let Some(service) = cluster.services.first() {
        if let Some(ref exec_start) = service.exec_start {
            // Parse exec_start to extract command and args
            let parts: Vec<&str> = exec_start.split_whitespace().collect();
            if !parts.is_empty() {
                let cmd_json: Vec<String> = parts.iter().map(|s| format!("\"{}\"", s)).collect();
                dockerfile.push_str(&format!("CMD [{}]\n", cmd_json.join(", ")));
            }
        }
    }

    Ok(dockerfile)
}

/// Generate entrypoint.sh script.
pub fn generate_entrypoint(cluster: &AppCluster) -> Result<String> {
    let mut script = String::new();

    script.push_str("#!/bin/bash\n");
    script.push_str("set -e\n\n");

    script.push_str("# Auto-generated entrypoint for ");
    script.push_str(&cluster.name);
    script.push_str("\n\n");

    // Render templates
    if cluster.config_files.iter().any(|c| c.templated) {
        script.push_str("# Render configuration templates\n");
        script.push_str("render_template() {\n");
        script.push_str("  local src=\"$1\"\n");
        script.push_str("  local dst=\"$2\"\n");
        script.push_str("  envsubst < \"$src\" > \"$dst\"\n");
        script.push_str("}\n\n");

        for config in &cluster.config_files {
            if config.templated {
                let filename = std::path::Path::new(&config.source_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "config".to_string());

                script.push_str(&format!(
                    "render_template /templates/{}.tmpl {}\n",
                    filename, config.container_path
                ));
            }
        }
        script.push('\n');
    }

    // Wait for dependencies
    if !cluster.depends_on.is_empty() || !cluster.external_deps.is_empty() {
        script.push_str("# Wait for dependencies\n");
        script.push_str("wait_for_port() {\n");
        script.push_str("  local host=\"$1\"\n");
        script.push_str("  local port=\"$2\"\n");
        script.push_str("  local retries=\"${3:-30}\"\n");
        script.push_str("  local wait=\"${4:-2}\"\n");
        script.push_str("  \n");
        script.push_str("  echo \"Waiting for $host:$port...\"\n");
        script.push_str("  for i in $(seq 1 $retries); do\n");
        script.push_str("    if nc -z \"$host\" \"$port\" 2>/dev/null; then\n");
        script.push_str("      echo \"$host:$port is available\"\n");
        script.push_str("      return 0\n");
        script.push_str("    fi\n");
        script.push_str("    sleep $wait\n");
        script.push_str("  done\n");
        script.push_str("  echo \"Timeout waiting for $host:$port\"\n");
        script.push_str("  return 1\n");
        script.push_str("}\n\n");

        // Add example wait calls (user needs to configure)
        script.push_str("# Example dependency waits (configure as needed):\n");
        for dep in &cluster.depends_on {
            script.push_str(&format!("# wait_for_port {} <port>\n", dep));
        }
        script.push('\n');
    }

    // Execute command
    script.push_str("# Execute the main command\n");
    script.push_str("exec \"$@\"\n");

    Ok(script)
}

/// Generate a config template.
pub fn generate_config_template(config: &ConfigFileSpec) -> Result<String> {
    let mut template = String::new();

    template.push_str("# Auto-generated template from ");
    template.push_str(&config.source_path);
    template.push('\n');
    template.push_str("#\n");
    template.push_str("# Template variables:\n");
    for var in &config.template_vars {
        template.push_str(&format!("#   ${{{}}}\n", var));
    }
    template.push_str("#\n");
    template.push_str("# Replace the content below with actual configuration,\n");
    template.push_str("# using ${VAR_NAME} syntax for templated values.\n\n");

    // Placeholder content
    for var in &config.template_vars {
        template.push_str(&format!("# {}=${{{}}}\n", var, var));
    }

    Ok(template)
}

/// Generate README for a cluster.
pub fn generate_readme(cluster: &AppCluster) -> Result<String> {
    let mut readme = String::new();

    readme.push_str(&format!("# {}\n\n", cluster.name));

    if let Some(ref desc) = cluster.description {
        readme.push_str(&format!("{}\n\n", desc));
    }

    readme.push_str("## Overview\n\n");
    readme.push_str(&format!("- **Type**: {}\n", cluster.app_type));
    readme.push_str(&format!(
        "- **Confidence**: {:.0}%\n",
        cluster.confidence * 100.0
    ));
    readme.push('\n');

    // Services
    if !cluster.services.is_empty() {
        readme.push_str("## Services\n\n");
        for service in &cluster.services {
            readme.push_str(&format!("- **{}**\n", service.name));
            if let Some(ref exec) = service.exec_start {
                readme.push_str(&format!("  - Command: `{}`\n", exec));
            }
            if let Some(ref user) = service.user {
                readme.push_str(&format!("  - User: `{}`\n", user));
            }
        }
        readme.push('\n');
    }

    // Ports
    if !cluster.ports.is_empty() {
        readme.push_str("## Ports\n\n");
        readme.push_str("| Port | Protocol | Purpose |\n");
        readme.push_str("|------|----------|--------|\n");
        for port in &cluster.ports {
            let purpose = port.purpose.as_deref().unwrap_or("Unknown");
            readme.push_str(&format!(
                "| {} | {} | {} |\n",
                port.port, port.protocol, purpose
            ));
        }
        readme.push('\n');
    }

    // Environment Variables
    if !cluster.env_vars.is_empty() {
        readme.push_str("## Environment Variables\n\n");
        readme.push_str("| Variable | Required | Sensitive | Description |\n");
        readme.push_str("|----------|----------|-----------|-------------|\n");
        for env in &cluster.env_vars {
            let required = if env.required { "Yes" } else { "No" };
            let sensitive = if env.sensitive { "Yes" } else { "No" };
            let desc = env.description.as_deref().unwrap_or("");
            readme.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                env.name, required, sensitive, desc
            ));
        }
        readme.push('\n');
    }

    // Config Files
    if !cluster.config_files.is_empty() {
        readme.push_str("## Configuration Files\n\n");
        for config in &cluster.config_files {
            readme.push_str(&format!(
                "- `{}` -> `{}`",
                config.source_path, config.container_path
            ));
            if config.templated {
                readme.push_str(" (templated)");
            }
            readme.push('\n');
        }
        readme.push('\n');
    }

    // Dependencies
    if !cluster.depends_on.is_empty() || !cluster.external_deps.is_empty() {
        readme.push_str("## Dependencies\n\n");
        if !cluster.depends_on.is_empty() {
            readme.push_str("### Internal Dependencies\n\n");
            for dep in &cluster.depends_on {
                readme.push_str(&format!("- {}\n", dep));
            }
            readme.push('\n');
        }
        if !cluster.external_deps.is_empty() {
            readme.push_str("### External Dependencies\n\n");
            for dep in &cluster.external_deps {
                readme.push_str(&format!("- {}\n", dep));
            }
            readme.push('\n');
        }
    }

    // Build & Run
    readme.push_str("## Build & Run\n\n");
    readme.push_str("```bash\n");
    readme.push_str("# Build the image\n");
    readme.push_str(&format!("docker build -t {} .\n\n", cluster.name));
    readme.push_str("# Run the container\n");
    readme.push_str("docker run -d");
    for port in &cluster.ports {
        readme.push_str(&format!(" -p {}:{}", port.port, port.port));
    }
    for env in &cluster.env_vars {
        if env.required && !env.sensitive {
            readme.push_str(&format!(" -e {}=<value>", env.name));
        }
    }
    readme.push_str(&format!(" {}\n", cluster.name));
    readme.push_str("```\n\n");

    // Notes
    readme.push_str("## Notes\n\n");
    readme.push_str("This Dockerfile was auto-generated by xcprobe analyzer.\n");
    readme.push_str("Review the following before production use:\n\n");
    readme.push_str("- [ ] Verify base image is appropriate\n");
    readme.push_str("- [ ] Add application files to the image\n");
    readme.push_str("- [ ] Configure environment variables\n");
    readme.push_str("- [ ] Review and adjust config templates\n");
    readme.push_str("- [ ] Set up proper health checks\n");
    readme.push_str("- [ ] Configure logging\n");

    Ok(readme)
}

/// Generate docker-compose.yaml for all clusters.
pub fn generate_compose(plan: &PackPlan) -> Result<String> {
    let mut compose = String::new();

    compose.push_str("# Auto-generated docker-compose.yaml\n");
    compose.push_str("# Generated by xcprobe analyzer\n\n");

    compose.push_str("services:\n");

    for cluster in &plan.clusters {
        compose.push_str(&format!("  {}:\n", cluster.id));
        compose.push_str("    build:\n");
        compose.push_str(&format!("      context: ./{}\n", cluster.id));
        compose.push_str("      dockerfile: Dockerfile\n");

        // Ports
        if !cluster.ports.is_empty() {
            compose.push_str("    ports:\n");
            for port in &cluster.ports {
                compose.push_str(&format!("      - \"{}:{}\"\n", port.port, port.port));
            }
        }

        // Environment
        if !cluster.env_vars.is_empty() {
            compose.push_str("    environment:\n");
            for env in &cluster.env_vars {
                if !env.sensitive {
                    if let Some(ref default) = env.default_value {
                        compose.push_str(&format!("      {}: \"{}\"\n", env.name, default));
                    } else {
                        compose.push_str(&format!("      {}: \"${{{}:-}}\"\n", env.name, env.name));
                    }
                }
            }
        }

        // Dependencies
        if !cluster.depends_on.is_empty() {
            compose.push_str("    depends_on:\n");
            for dep in &cluster.depends_on {
                compose.push_str(&format!("      {}:\n", dep));
                compose.push_str("        condition: service_healthy\n");
            }
        }

        // Healthcheck
        if !cluster.ports.is_empty() {
            let port = cluster.ports[0].port;
            compose.push_str("    healthcheck:\n");
            compose.push_str(&format!(
                "      test: [\"CMD\", \"nc\", \"-z\", \"localhost\", \"{}\"]\n",
                port
            ));
            compose.push_str("      interval: 10s\n");
            compose.push_str("      timeout: 5s\n");
            compose.push_str("      retries: 3\n");
        }

        compose.push('\n');
    }

    Ok(compose)
}

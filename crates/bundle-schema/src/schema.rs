//! JSON schema definitions for bundle validation.

/// JSON Schema for manifest.json.
pub const MANIFEST_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://xcprobe.dev/schemas/manifest.json",
  "title": "XCProbe Bundle Manifest",
  "type": "object",
  "required": ["schema_version", "collection_id", "collected_at", "system", "processes", "services", "ports"],
  "properties": {
    "schema_version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "collection_id": {
      "type": "string",
      "format": "uuid"
    },
    "collected_at": {
      "type": "string",
      "format": "date-time"
    },
    "completed_at": {
      "type": ["string", "null"],
      "format": "date-time"
    },
    "system": {
      "type": "object",
      "required": ["hostname", "os_type"],
      "properties": {
        "hostname": { "type": "string" },
        "os_type": { "type": "string", "enum": ["linux", "windows"] },
        "os_version": { "type": ["string", "null"] },
        "kernel_version": { "type": ["string", "null"] },
        "architecture": { "type": ["string", "null"] },
        "uptime_seconds": { "type": ["integer", "null"] },
        "timezone": { "type": ["string", "null"] }
      }
    },
    "processes": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["pid", "ppid", "user", "command", "full_cmdline"],
        "properties": {
          "pid": { "type": "integer" },
          "ppid": { "type": "integer" },
          "user": { "type": "string" },
          "command": { "type": "string" },
          "args": { "type": "array", "items": { "type": "string" } },
          "full_cmdline": { "type": "string" },
          "start_time": { "type": ["string", "null"] },
          "elapsed_time": { "type": ["string", "null"] },
          "working_directory": { "type": ["string", "null"] },
          "evidence_ref": { "type": ["string", "null"] }
        }
      }
    },
    "services": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "state"],
        "properties": {
          "name": { "type": "string" },
          "display_name": { "type": ["string", "null"] },
          "state": { "type": "string" },
          "sub_state": { "type": ["string", "null"] },
          "start_mode": { "type": ["string", "null"] },
          "exec_start": { "type": ["string", "null"] },
          "working_directory": { "type": ["string", "null"] },
          "user": { "type": ["string", "null"] },
          "environment": { "type": "object" },
          "environment_files": { "type": "array", "items": { "type": "string" } },
          "unit_file_path": { "type": ["string", "null"] },
          "evidence_ref": { "type": ["string", "null"] }
        }
      }
    },
    "ports": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["protocol", "local_address", "local_port", "state"],
        "properties": {
          "protocol": { "type": "string" },
          "local_address": { "type": "string" },
          "local_port": { "type": "integer" },
          "state": { "type": "string" },
          "pid": { "type": ["integer", "null"] },
          "process_name": { "type": ["string", "null"] },
          "evidence_ref": { "type": ["string", "null"] }
        }
      }
    },
    "connections": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "protocol": { "type": "string" },
          "local_address": { "type": "string" },
          "local_port": { "type": "integer" },
          "remote_address": { "type": "string" },
          "remote_port": { "type": "integer" },
          "state": { "type": "string" },
          "pid": { "type": ["integer", "null"] }
        }
      }
    },
    "packages": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "version", "source"],
        "properties": {
          "name": { "type": "string" },
          "version": { "type": "string" },
          "architecture": { "type": ["string", "null"] },
          "source": { "type": "string" }
        }
      }
    },
    "scheduled_tasks": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "task_type", "enabled"],
        "properties": {
          "name": { "type": "string" },
          "task_type": { "type": "string" },
          "schedule": { "type": ["string", "null"] },
          "command": { "type": ["string", "null"] },
          "enabled": { "type": "boolean" }
        }
      }
    },
    "config_files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "size_bytes", "discovery_method"],
        "properties": {
          "path": { "type": "string" },
          "size_bytes": { "type": "integer" },
          "modified_at": { "type": ["string", "null"] },
          "attachment_ref": { "type": ["string", "null"] },
          "discovery_method": { "type": "string" }
        }
      }
    },
    "log_files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "size_bytes", "discovery_method"],
        "properties": {
          "path": { "type": "string" },
          "size_bytes": { "type": "integer" },
          "attachment_ref": { "type": ["string", "null"] },
          "discovery_method": { "type": "string" }
        }
      }
    },
    "environment_files": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "variable_names"],
        "properties": {
          "path": { "type": "string" },
          "variable_names": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "collection_mode": { "type": "string" },
    "errors": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["phase", "error", "timestamp", "recoverable"],
        "properties": {
          "phase": { "type": "string" },
          "command": { "type": ["string", "null"] },
          "error": { "type": "string" },
          "timestamp": { "type": "string" },
          "recoverable": { "type": "boolean" }
        }
      }
    }
  }
}"#;

/// JSON Schema for packplan.json.
pub const PACKPLAN_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://xcprobe.dev/schemas/packplan.json",
  "title": "XCProbe Pack Plan",
  "type": "object",
  "required": ["schema_version", "generated_at", "source_bundle_id", "clusters", "overall_confidence"],
  "properties": {
    "schema_version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "generated_at": {
      "type": "string",
      "format": "date-time"
    },
    "source_bundle_id": {
      "type": "string"
    },
    "clusters": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "name", "app_type", "confidence", "evidence_refs", "decisions"],
        "properties": {
          "id": { "type": "string" },
          "name": { "type": "string" },
          "app_type": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
          "evidence_refs": {
            "type": "array",
            "items": { "type": "string" }
          },
          "decisions": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["decision", "reason", "evidence_refs", "confidence"],
              "properties": {
                "decision": { "type": "string" },
                "reason": { "type": "string" },
                "evidence_refs": {
                  "type": "array",
                  "items": { "type": "string" }
                },
                "confidence": { "type": "number" }
              }
            }
          }
        }
      }
    },
    "overall_confidence": {
      "type": "number",
      "minimum": 0,
      "maximum": 1
    }
  }
}"#;

/// Get the manifest schema as a parsed JSON value.
pub fn manifest_schema() -> serde_json::Value {
    serde_json::from_str(MANIFEST_SCHEMA).expect("Invalid manifest schema")
}

/// Get the packplan schema as a parsed JSON value.
pub fn packplan_schema() -> serde_json::Value {
    serde_json::from_str(PACKPLAN_SCHEMA).expect("Invalid packplan schema")
}

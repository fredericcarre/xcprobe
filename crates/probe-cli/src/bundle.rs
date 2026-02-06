//! Bundle reading and writing utilities.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tar::{Archive, Builder};
use tracing::info;
use xcprobe_bundle_schema::{validation, Bundle, Evidence, Manifest};

/// Write a bundle to a compressed tarball.
pub fn write_bundle(bundle: &Bundle, path: &Path) -> Result<()> {
    let file = File::create(path).context("Failed to create bundle file")?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);

    // Write manifest.json
    let manifest_json = serde_json::to_string_pretty(&bundle.manifest)?;
    add_file_to_archive(&mut archive, "manifest.json", manifest_json.as_bytes())?;

    // Write audit.jsonl
    let audit_content: Vec<String> = bundle
        .audit
        .iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .collect();
    let audit_jsonl = audit_content.join("\n");
    add_file_to_archive(&mut archive, "audit.jsonl", audit_jsonl.as_bytes())?;

    // Write evidence files
    for (path, evidence) in &bundle.evidence {
        if let Some(ref content) = evidence.content {
            add_file_to_archive(&mut archive, path, content)?;
        }
    }

    // Write checksums.json
    let checksums_json = serde_json::to_string_pretty(&bundle.checksums)?;
    add_file_to_archive(&mut archive, "checksums.json", checksums_json.as_bytes())?;

    archive.finish()?;
    info!("Bundle written successfully");

    Ok(())
}

fn add_file_to_archive<W: Write>(
    archive: &mut Builder<W>,
    path: &str,
    content: &[u8],
) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    archive.append_data(&mut header, path, content)?;
    Ok(())
}

/// Read a bundle from a compressed tarball.
pub fn read_bundle(path: &Path) -> Result<Bundle> {
    let file = File::open(path).context("Failed to open bundle file")?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut manifest: Option<Manifest> = None;
    let mut audit = Vec::new();
    let mut evidence: HashMap<String, Evidence> = HashMap::new();
    let mut checksums: HashMap<String, String> = HashMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();

        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        if path == "manifest.json" {
            manifest = Some(serde_json::from_slice(&content)?);
        } else if path == "audit.jsonl" {
            let content_str = String::from_utf8_lossy(&content);
            for line in content_str.lines() {
                if !line.trim().is_empty() {
                    if let Ok(entry) = serde_json::from_str(line) {
                        audit.push(entry);
                    }
                }
            }
        } else if path == "checksums.json" {
            checksums = serde_json::from_slice(&content)?;
        } else if path.starts_with("evidence/") || path.starts_with("attachments/") {
            let hash = xcprobe_common::hash::sha256_bytes(&content);
            let ev = Evidence {
                id: path.clone(),
                evidence_type: xcprobe_bundle_schema::EvidenceType::CommandOutput,
                collected_at: chrono::Utc::now(),
                source_command: None,
                size_bytes: content.len() as u64,
                content_hash: hash,
                redacted: false,
                bundle_path: path.clone(),
                original_path: None,
                content: Some(content),
            };
            evidence.insert(path, ev);
        }
    }

    Ok(Bundle {
        manifest: manifest.context("Missing manifest.json in bundle")?,
        audit,
        evidence,
        checksums,
    })
}

/// Validate a bundle file.
pub fn validate_bundle_file(
    path: &Path,
    check_evidence: bool,
    verify_checksums: bool,
) -> Result<validation::ValidationResult> {
    let bundle = read_bundle(path)?;

    let evidence_files: HashSet<String> = bundle.evidence.keys().cloned().collect();

    let mut result =
        validation::validate_bundle(&bundle.manifest, &evidence_files, &bundle.checksums)?;

    // Verify checksums
    if verify_checksums {
        for (path, expected_hash) in &bundle.checksums {
            if let Some(evidence) = bundle.evidence.get(path) {
                if evidence.content_hash != *expected_hash {
                    result.add_error(validation::ValidationError::ChecksumMismatch {
                        file: path.clone(),
                        expected: expected_hash.clone(),
                        actual: evidence.content_hash.clone(),
                    });
                }
            }
        }
    }

    // Check evidence references exist
    if check_evidence {
        // Already done in validate_bundle
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_read_bundle() {
        let dir = tempdir().unwrap();
        let bundle_path = dir.path().join("test.tgz");

        let manifest = Manifest::default();
        let bundle = Bundle {
            manifest,
            audit: vec![],
            evidence: HashMap::new(),
            checksums: HashMap::new(),
        };

        write_bundle(&bundle, &bundle_path).unwrap();
        let read_bundle = read_bundle(&bundle_path).unwrap();

        assert_eq!(read_bundle.manifest.schema_version, "1.0.0");
    }
}

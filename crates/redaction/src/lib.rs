//! Secret redaction for xcprobe bundles.
//!
//! This crate provides functions to detect and redact sensitive information
//! from collected data before it is written to the bundle.

pub mod entropy;
pub mod patterns;
pub mod redactor;

pub use redactor::{Redactor, RedactionResult, RedactionStats};

/// Default placeholder for redacted values.
pub const REDACTED_PLACEHOLDER: &str = "[REDACTED]";

/// Hash-only placeholder (shows hash of original value).
pub const HASH_PLACEHOLDER_PREFIX: &str = "[HASH:";
pub const HASH_PLACEHOLDER_SUFFIX: &str = "]";

/// Create a hash placeholder for a value.
pub fn hash_placeholder(value: &str) -> String {
    let hash = xcprobe_common::hash::sha256_str(value);
    format!("{}{}{}", HASH_PLACEHOLDER_PREFIX, &hash[..12], HASH_PLACEHOLDER_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_placeholder() {
        let placeholder = hash_placeholder("my-secret-value");
        assert!(placeholder.starts_with(HASH_PLACEHOLDER_PREFIX));
        assert!(placeholder.ends_with(HASH_PLACEHOLDER_SUFFIX));
        assert_eq!(placeholder.len(), 20); // [HASH: + 12 chars + ]
    }
}

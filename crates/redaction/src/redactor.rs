//! Main redactor implementation.

use crate::entropy::{looks_like_token, DEFAULT_ENTROPY_THRESHOLD};
use crate::patterns::{self, is_sensitive_key};
use crate::{hash_placeholder, REDACTED_PLACEHOLDER};
use regex::Regex;
use tracing::{debug, trace};

/// Configuration for the redactor.
#[derive(Debug, Clone)]
pub struct RedactorConfig {
    /// Use hash placeholders instead of generic [REDACTED].
    pub use_hash_placeholders: bool,
    /// Entropy threshold for high-entropy detection.
    pub entropy_threshold: f64,
    /// Enable entropy-based detection.
    pub enable_entropy_detection: bool,
    /// Additional patterns to match (regex strings).
    pub additional_patterns: Vec<String>,
}

impl Default for RedactorConfig {
    fn default() -> Self {
        Self {
            use_hash_placeholders: false,
            entropy_threshold: DEFAULT_ENTROPY_THRESHOLD,
            enable_entropy_detection: true,
            additional_patterns: Vec::new(),
        }
    }
}

/// Result of redaction.
#[derive(Debug)]
pub struct RedactionResult {
    /// The redacted content.
    pub content: String,
    /// Statistics about what was redacted.
    pub stats: RedactionStats,
}

/// Statistics about redactions performed.
#[derive(Debug, Default, Clone)]
pub struct RedactionStats {
    /// Number of pattern-based redactions.
    pub pattern_redactions: usize,
    /// Number of entropy-based redactions.
    pub entropy_redactions: usize,
    /// Number of sensitive key redactions.
    pub key_redactions: usize,
    /// Total characters redacted.
    pub total_chars_redacted: usize,
    /// Names of patterns that matched.
    pub matched_patterns: Vec<String>,
}

impl RedactionStats {
    /// Total number of redactions.
    pub fn total(&self) -> usize {
        self.pattern_redactions + self.entropy_redactions + self.key_redactions
    }

    /// Merge with another stats.
    pub fn merge(&mut self, other: &RedactionStats) {
        self.pattern_redactions += other.pattern_redactions;
        self.entropy_redactions += other.entropy_redactions;
        self.key_redactions += other.key_redactions;
        self.total_chars_redacted += other.total_chars_redacted;
        self.matched_patterns
            .extend(other.matched_patterns.iter().cloned());
    }
}

/// The main redactor.
pub struct Redactor {
    config: RedactorConfig,
    additional_patterns: Vec<Regex>,
}

impl Redactor {
    /// Create a new redactor with default configuration.
    pub fn new() -> Self {
        Self::with_config(RedactorConfig::default())
    }

    /// Create a new redactor with custom configuration.
    pub fn with_config(config: RedactorConfig) -> Self {
        let additional_patterns = config
            .additional_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            config,
            additional_patterns,
        }
    }

    /// Redact sensitive content from a string.
    pub fn redact(&self, content: &str) -> RedactionResult {
        let mut stats = RedactionStats::default();
        let mut result = content.to_string();

        // Apply pattern-based redaction
        for (name, pattern) in patterns::all_redaction_patterns() {
            let before_len = result.len();
            result = self.apply_pattern_redaction(&result, pattern, &mut stats);
            if result.len() != before_len {
                stats.matched_patterns.push(name.to_string());
            }
        }

        // Apply additional patterns
        for pattern in &self.additional_patterns {
            result = self.apply_pattern_redaction(&result, pattern, &mut stats);
        }

        // Apply entropy-based detection on remaining potential tokens
        if self.config.enable_entropy_detection {
            result = self.apply_entropy_redaction(&result, &mut stats);
        }

        RedactionResult {
            content: result,
            stats,
        }
    }

    /// Redact a key-value pair (for environment variables, config keys, etc.).
    pub fn redact_key_value(&self, key: &str, value: &str) -> (String, RedactionStats) {
        let mut stats = RedactionStats::default();

        if is_sensitive_key(key) {
            stats.key_redactions += 1;
            stats.total_chars_redacted += value.len();
            let placeholder = if self.config.use_hash_placeholders {
                hash_placeholder(value)
            } else {
                REDACTED_PLACEHOLDER.to_string()
            };
            return (placeholder, stats);
        }

        // Check if value looks like a token
        if self.config.enable_entropy_detection && looks_like_token(value) {
            stats.entropy_redactions += 1;
            stats.total_chars_redacted += value.len();
            let placeholder = if self.config.use_hash_placeholders {
                hash_placeholder(value)
            } else {
                REDACTED_PLACEHOLDER.to_string()
            };
            return (placeholder, stats);
        }

        (value.to_string(), stats)
    }

    /// Apply pattern-based redaction.
    fn apply_pattern_redaction(
        &self,
        content: &str,
        pattern: &Regex,
        stats: &mut RedactionStats,
    ) -> String {
        let mut result = content.to_string();
        let mut offset: i64 = 0;

        for mat in pattern.find_iter(content) {
            let matched = mat.as_str();
            let start = (mat.start() as i64 + offset) as usize;
            let end = (mat.end() as i64 + offset) as usize;

            let replacement = if self.config.use_hash_placeholders {
                hash_placeholder(matched)
            } else {
                REDACTED_PLACEHOLDER.to_string()
            };

            stats.pattern_redactions += 1;
            stats.total_chars_redacted += matched.len();

            let len_diff = replacement.len() as i64 - matched.len() as i64;
            result.replace_range(start..end, &replacement);
            offset += len_diff;

            trace!(
                "Redacted pattern match: {} -> {}",
                matched.len(),
                replacement.len()
            );
        }

        result
    }

    /// Apply entropy-based redaction to potential tokens.
    fn apply_entropy_redaction(&self, content: &str, stats: &mut RedactionStats) -> String {
        // Split content into words and check each for high entropy
        let mut result = String::with_capacity(content.len());
        let mut current_word = String::new();

        for c in content.chars() {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '+' || c == '/' || c == '=' {
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    if looks_like_token(&current_word) {
                        let replacement = if self.config.use_hash_placeholders {
                            hash_placeholder(&current_word)
                        } else {
                            REDACTED_PLACEHOLDER.to_string()
                        };
                        stats.entropy_redactions += 1;
                        stats.total_chars_redacted += current_word.len();
                        result.push_str(&replacement);
                        debug!("Entropy redaction: {} chars", current_word.len());
                    } else {
                        result.push_str(&current_word);
                    }
                    current_word.clear();
                }
                result.push(c);
            }
        }

        // Handle last word
        if !current_word.is_empty() {
            if looks_like_token(&current_word) {
                let replacement = if self.config.use_hash_placeholders {
                    hash_placeholder(&current_word)
                } else {
                    REDACTED_PLACEHOLDER.to_string()
                };
                stats.entropy_redactions += 1;
                stats.total_chars_redacted += current_word.len();
                result.push_str(&replacement);
            } else {
                result.push_str(&current_word);
            }
        }

        result
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_password_env_var() {
        let redactor = Redactor::new();
        let result = redactor.redact("DATABASE_PASSWORD=mysecret123");
        assert!(
            result.content.contains(REDACTED_PLACEHOLDER)
                || !result.content.contains("mysecret123")
        );
        assert!(result.stats.total() > 0);
    }

    #[test]
    fn test_redact_connection_string() {
        let redactor = Redactor::new();
        let result = redactor.redact("db_url=postgres://user:password123@localhost:5432/mydb");
        assert!(!result.content.contains("password123"));
    }

    #[test]
    fn test_redact_auth_header() {
        let redactor = Redactor::new();
        let result =
            redactor.redact("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWI");
        assert!(!result
            .content
            .contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_redact_key_value_sensitive() {
        let redactor = Redactor::new();
        let (value, stats) = redactor.redact_key_value("API_KEY", "sk-1234567890abcdef");
        assert_eq!(value, REDACTED_PLACEHOLDER);
        assert_eq!(stats.key_redactions, 1);
    }

    #[test]
    fn test_redact_key_value_non_sensitive() {
        let redactor = Redactor::new();
        let (value, stats) = redactor.redact_key_value("LOG_LEVEL", "debug");
        assert_eq!(value, "debug");
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_hash_placeholder_mode() {
        let config = RedactorConfig {
            use_hash_placeholders: true,
            ..Default::default()
        };
        let redactor = Redactor::with_config(config);
        let (value, _) = redactor.redact_key_value("PASSWORD", "mysecret");
        assert!(value.starts_with("[HASH:"));
        assert!(value.ends_with("]"));
    }

    #[test]
    fn test_no_secret_passes_through() {
        let redactor = Redactor::new();
        let result = redactor.redact("This is a normal log message with no secrets");
        assert_eq!(
            result.content,
            "This is a normal log message with no secrets"
        );
        assert_eq!(result.stats.total(), 0);
    }

    #[test]
    fn test_aws_key_redaction() {
        let redactor = Redactor::new();
        let result = redactor.redact("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE");
        assert!(!result.content.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_json_secret_redaction() {
        let redactor = Redactor::new();
        let result = redactor.redact(r#"{"password": "super_secret_123", "username": "admin"}"#);
        assert!(!result.content.contains("super_secret_123"));
        assert!(result.content.contains("admin")); // username should not be redacted
    }
}

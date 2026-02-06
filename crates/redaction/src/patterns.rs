//! Regex patterns for detecting sensitive data.

use regex::Regex;
use std::sync::LazyLock;

/// Pattern for common secret key names in environment variables and configs.
pub static SECRET_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(password|passwd|pwd|secret|token|api[_-]?key|apikey|auth[_-]?token|access[_-]?token|private[_-]?key|client[_-]?secret|bearer|credentials?|jwt|session[_-]?id|cookie|oauth)").unwrap()
});

/// Pattern for Authorization headers.
pub static AUTH_HEADER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(Authorization|X-Api-Key|X-Auth-Token|X-Access-Token):\s*\S+").unwrap()
});

/// Pattern for connection strings.
pub static CONNECTION_STRING_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(mongodb|mysql|postgres|postgresql|redis|amqp|mssql)://[^\s]+").unwrap()
});

/// Pattern for database URLs with credentials.
pub static DB_URL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[a-z]+://[^:]+:[^@]+@[^\s]+").unwrap());

/// Pattern for AWS access keys.
pub static AWS_KEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}").unwrap());

/// Pattern for AWS secret keys.
pub static AWS_SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[=:]\s*[A-Za-z0-9/+=]{40}").unwrap()
});

/// Pattern for generic API keys (base64-ish strings after key= or key:).
pub static GENERIC_API_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(api[_-]?key|token|secret)\s*[=:]\s*[A-Za-z0-9_\-+/=]{20,}").unwrap()
});

/// Pattern for private keys (PEM format).
pub static PRIVATE_KEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----").unwrap());

/// Pattern for environment variable assignments with sensitive names.
pub static ENV_VAR_ASSIGNMENT_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^([A-Z_][A-Z0-9_]*(?:PASSWORD|PASSWD|PWD|SECRET|TOKEN|API[_-]?KEY|APIKEY|AUTH|PRIVATE|CREDENTIALS?)[A-Z0-9_]*)\s*=\s*(.+)$").unwrap()
});

/// Pattern for JSON/YAML sensitive keys.
pub static JSON_SENSITIVE_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)"(password|secret|token|api[_-]?key|private[_-]?key|credentials?)"\s*:\s*"([^"]+)""#,
    )
    .unwrap()
});

/// Pattern for YAML sensitive keys.
pub static YAML_SENSITIVE_KEY_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(\s*)(password|secret|token|api[_-]?key|private[_-]?key|credentials?):\s*(.+)$",
    )
    .unwrap()
});

/// Pattern for IP addresses (for potential redaction if needed).
pub static IP_ADDRESS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

/// Check if a key name looks like it contains a secret.
pub fn is_sensitive_key(key: &str) -> bool {
    SECRET_KEY_PATTERN.is_match(key)
}

/// All patterns that should trigger redaction.
pub fn all_redaction_patterns() -> Vec<(&'static str, &'static Regex)> {
    vec![
        ("auth_header", &AUTH_HEADER_PATTERN),
        ("connection_string", &CONNECTION_STRING_PATTERN),
        ("db_url", &DB_URL_PATTERN),
        ("aws_key", &AWS_KEY_PATTERN),
        ("aws_secret", &AWS_SECRET_PATTERN),
        ("generic_api_key", &GENERIC_API_KEY_PATTERN),
        ("private_key", &PRIVATE_KEY_PATTERN),
        ("env_var_assignment", &ENV_VAR_ASSIGNMENT_PATTERN),
        ("json_sensitive", &JSON_SENSITIVE_KEY_PATTERN),
        ("yaml_sensitive", &YAML_SENSITIVE_KEY_PATTERN),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_key() {
        assert!(is_sensitive_key("DATABASE_PASSWORD"));
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("API_KEY"));
        assert!(is_sensitive_key("secret_token"));
        assert!(is_sensitive_key("OAUTH_TOKEN"));
        assert!(is_sensitive_key("private_key"));

        assert!(!is_sensitive_key("DATABASE_HOST"));
        assert!(!is_sensitive_key("PORT"));
        assert!(!is_sensitive_key("LOG_LEVEL"));
    }

    #[test]
    fn test_connection_string_pattern() {
        assert!(CONNECTION_STRING_PATTERN.is_match("mongodb://user:pass@localhost:27017/db"));
        assert!(CONNECTION_STRING_PATTERN.is_match("postgres://admin:secret@db.example.com/mydb"));
        assert!(CONNECTION_STRING_PATTERN.is_match("redis://localhost:6379"));
    }

    #[test]
    fn test_aws_key_pattern() {
        assert!(AWS_KEY_PATTERN.is_match("AKIAIOSFODNN7EXAMPLE"));
        assert!(!AWS_KEY_PATTERN.is_match("NOTANAWSKEY12345678"));
    }

    #[test]
    fn test_auth_header_pattern() {
        assert!(AUTH_HEADER_PATTERN.is_match("Authorization: Bearer eyJhbGciOiJIUzI1NiIs"));
        assert!(AUTH_HEADER_PATTERN.is_match("X-Api-Key: sk-1234567890abcdef"));
    }

    #[test]
    fn test_env_var_assignment() {
        assert!(ENV_VAR_ASSIGNMENT_PATTERN.is_match("DATABASE_PASSWORD=mysecret123"));
        assert!(ENV_VAR_ASSIGNMENT_PATTERN.is_match("API_KEY=abcdef12345"));
        assert!(ENV_VAR_ASSIGNMENT_PATTERN.is_match("JWT_SECRET=supersecret"));
    }

    #[test]
    fn test_json_sensitive() {
        assert!(JSON_SENSITIVE_KEY_PATTERN.is_match(r#""password": "secret123""#));
        assert!(JSON_SENSITIVE_KEY_PATTERN.is_match(r#""api_key": "sk-12345""#));
    }
}

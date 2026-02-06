//! Entropy-based detection of high-entropy strings (likely secrets).

use std::collections::HashMap;

/// Calculate Shannon entropy of a string.
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }

    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in s.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }

    let len = s.len() as f64;
    freq.values()
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Check if a string has high entropy (likely a secret/token).
///
/// Thresholds are based on typical secret patterns:
/// - Random hex strings: ~4.0 bits/char
/// - Base64 strings: ~5.0-6.0 bits/char
/// - English words: ~1.0-2.0 bits/char
pub fn is_high_entropy(s: &str, threshold: f64) -> bool {
    // Only check strings of reasonable length for secrets
    if s.len() < 16 || s.len() > 256 {
        return false;
    }

    // Skip strings that look like paths or URLs without credentials
    if s.starts_with('/') || s.starts_with("http://") || s.starts_with("https://") {
        // But still check if it contains credentials
        if !s.contains('@') && !s.contains("://") {
            return false;
        }
    }

    shannon_entropy(s) >= threshold
}

/// Default entropy threshold for secret detection.
pub const DEFAULT_ENTROPY_THRESHOLD: f64 = 4.0;

/// Check if a string looks like a secret token.
pub fn looks_like_token(s: &str) -> bool {
    // Must be at least 16 chars
    if s.len() < 16 {
        return false;
    }

    // Check character composition
    let alphanumeric_count = s.chars().filter(|c| c.is_alphanumeric()).count();
    let alphanumeric_ratio = alphanumeric_count as f64 / s.len() as f64;

    // Tokens are usually mostly alphanumeric with some symbols
    if alphanumeric_ratio < 0.7 {
        return false;
    }

    // Check entropy
    is_high_entropy(s, DEFAULT_ENTROPY_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy() {
        // Low entropy - repeated character
        let low = shannon_entropy("aaaaaaaaaa");
        assert!(low < 1.0);

        // High entropy - random-looking string
        let high = shannon_entropy("aB3dE5fG7hI9jK1lM3nO5pQ7rS9tU1vW3");
        assert!(high > 4.0);

        // Medium entropy - English word
        let medium = shannon_entropy("password");
        assert!(medium > 2.0 && medium < 4.0);
    }

    #[test]
    fn test_is_high_entropy() {
        // High entropy strings
        assert!(is_high_entropy(
            "sk_live_1234567890abcdefghij",
            DEFAULT_ENTROPY_THRESHOLD
        ));
        assert!(is_high_entropy(
            "aB3dE5fG7hI9jK1lM3nO5pQ7rS9tU1vW3",
            DEFAULT_ENTROPY_THRESHOLD
        ));

        // Low entropy strings
        assert!(!is_high_entropy(
            "passwordpasswordpassword",
            DEFAULT_ENTROPY_THRESHOLD
        ));
        assert!(!is_high_entropy(
            "aaaaaaaaaaaaaaaa",
            DEFAULT_ENTROPY_THRESHOLD
        ));

        // Too short
        assert!(!is_high_entropy("short", DEFAULT_ENTROPY_THRESHOLD));
    }

    #[test]
    fn test_looks_like_token() {
        assert!(looks_like_token("sk_live_1234567890abcdefghij"));
        assert!(looks_like_token("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));

        assert!(!looks_like_token("hello world this is a sentence"));
        assert!(!looks_like_token("short"));
    }
}

//! Timestamp utilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A wrapper around DateTime<Utc> with consistent serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Create a new timestamp from the current time.
    pub fn now() -> Self {
        Timestamp(Utc::now())
    }

    /// Create a timestamp from a DateTime<Utc>.
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Timestamp(dt)
    }

    /// Get the inner DateTime<Utc>.
    pub fn inner(&self) -> DateTime<Utc> {
        self.0
    }

    /// Format as ISO 8601 string.
    pub fn to_iso8601(&self) -> String {
        self.0.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }

    /// Format for journalctl --since.
    pub fn to_journalctl_format(&self) -> String {
        self.0.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// Duration since this timestamp.
    pub fn elapsed(&self) -> chrono::Duration {
        Utc::now() - self.0
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_iso8601())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_serialization() {
        let ts = Timestamp::now();
        let json = serde_json::to_string(&ts).unwrap();
        let parsed: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(ts, parsed);
    }
}

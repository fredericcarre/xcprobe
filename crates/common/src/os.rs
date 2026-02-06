//! Operating system type definitions.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Target operating system type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsType {
    Linux,
    Windows,
}

impl fmt::Display for OsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OsType::Linux => write!(f, "linux"),
            OsType::Windows => write!(f, "windows"),
        }
    }
}

impl FromStr for OsType {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "linux" => Ok(OsType::Linux),
            "windows" => Ok(OsType::Windows),
            _ => Err(crate::Error::UnsupportedOs(s.to_string())),
        }
    }
}

impl OsType {
    /// Check if the OS is Linux.
    pub fn is_linux(&self) -> bool {
        matches!(self, OsType::Linux)
    }

    /// Check if the OS is Windows.
    pub fn is_windows(&self) -> bool {
        matches!(self, OsType::Windows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_os_type() {
        assert_eq!(OsType::from_str("linux").unwrap(), OsType::Linux);
        assert_eq!(OsType::from_str("Linux").unwrap(), OsType::Linux);
        assert_eq!(OsType::from_str("LINUX").unwrap(), OsType::Linux);
        assert_eq!(OsType::from_str("windows").unwrap(), OsType::Windows);
        assert_eq!(OsType::from_str("Windows").unwrap(), OsType::Windows);
    }

    #[test]
    fn test_display_os_type() {
        assert_eq!(OsType::Linux.to_string(), "linux");
        assert_eq!(OsType::Windows.to_string(), "windows");
    }
}

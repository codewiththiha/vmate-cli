//! Country codes and their validation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// A validated, uppercase two-letter ISO country code.
///
/// The special value `UNKNOWN` is allowed for configs whose origin cannot
/// be determined. All values are stored and compared uppercase, so matching
/// is inherently case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CountryCode(String);

impl CountryCode {
    /// Construct a `CountryCode`, normalizing case and validating the input.
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self, CountryError> {
        Self::from_str(s.as_ref())
    }

    /// The normalized uppercase code (e.g. `JP`).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The `UNKNOWN` sentinel.
    pub fn unknown() -> Self {
        CountryCode("UNKNOWN".to_string())
    }

    /// Whether this code is the `UNKNOWN` sentinel.
    pub fn is_unknown(&self) -> bool {
        self.0 == "UNKNOWN"
    }
}

impl FromStr for CountryCode {
    type Err = CountryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_uppercase();

        if normalized.is_empty() {
            return Err(CountryError::Empty);
        }

        if normalized == "UNKNOWN" {
            return Ok(CountryCode(normalized));
        }

        if normalized.len() != 2 || !normalized.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(CountryError::Invalid(normalized));
        }

        Ok(CountryCode(normalized))
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors produced while parsing a country code.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CountryError {
    #[error("country code cannot be empty")]
    Empty,
    #[error("invalid country code: {0:?}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_to_uppercase() {
        assert_eq!(CountryCode::new("jp").unwrap().as_str(), "JP");
        assert_eq!(CountryCode::new("kr").unwrap().as_str(), "KR");
        assert_eq!(CountryCode::new("US").unwrap().as_str(), "US");
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(CountryCode::new("  jp ").unwrap().as_str(), "JP");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(CountryCode::new(""), Err(CountryError::Empty));
        assert_eq!(CountryCode::new("   "), Err(CountryError::Empty));
    }

    #[test]
    fn rejects_invalid() {
        assert!(CountryCode::new("JPNX").is_err());
        assert!(CountryCode::new("J1").is_err());
        assert!(CountryCode::new("jp-kr").is_err());
    }

    #[test]
    fn accepts_unknown() {
        let code = CountryCode::new("unknown").unwrap();
        assert!(code.is_unknown());
        assert_eq!(code.as_str(), "UNKNOWN");
    }

    #[test]
    fn equality_is_upper() {
        assert_eq!(
            CountryCode::new("jp").unwrap(),
            CountryCode::new("JP").unwrap()
        );
    }
}

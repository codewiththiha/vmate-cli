//! Country filtering.

use crate::country::{CountryCode, CountryError};
use std::collections::HashSet;
use std::str::FromStr;

/// A set of country codes used to restrict results.
///
/// An empty filter matches everything; a non-empty filter matches only the
/// listed countries. Matching is case-insensitive because every stored code
/// is normalized to uppercase.
#[derive(Debug, Clone, Default)]
pub struct CountryFilter {
    countries: HashSet<CountryCode>,
}

impl CountryFilter {
    /// An empty filter — matches all countries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a filter from raw CLI values.
    ///
    /// Handles comma-separated values and repeated flags:
    /// `["jp,kr", "us"]` -> `{JP, KR, US}`. Empty segments are ignored.
    pub fn from_args(values: &[String]) -> Result<Self, CountryError> {
        let mut filter = Self::new();
        for value in values {
            for part in value.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                filter.add(part)?;
            }
        }
        Ok(filter)
    }

    /// Add a single country to the filter.
    pub fn add(&mut self, value: &str) -> Result<(), CountryError> {
        let code = CountryCode::from_str(value)?;
        self.countries.insert(code);
        Ok(())
    }

    /// True when the filter is empty (matches everything).
    pub fn is_empty(&self) -> bool {
        self.countries.is_empty()
    }

    /// Iterate over the contained country codes.
    pub fn countries(&self) -> impl Iterator<Item = &CountryCode> {
        self.countries.iter()
    }

    /// Whether the given country is allowed by this filter.
    pub fn matches(&self, country: &str) -> bool {
        if self.countries.is_empty() {
            return true;
        }
        let normalized = country.trim().to_ascii_uppercase();
        self.countries
            .iter()
            .any(|code| code.as_str() == normalized)
    }

    /// A human readable summary, e.g. `JP, KR` or `ALL`.
    pub fn to_display(&self) -> String {
        if self.countries.is_empty() {
            return "ALL".to_string();
        }
        let mut codes: Vec<String> = self.countries.iter().map(|c| c.to_string()).collect();
        codes.sort();
        codes.join(", ")
    }
}

impl std::fmt::Display for CountryFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_is_case_insensitive() {
        let filter = CountryFilter::from_args(&["jp,kr".to_string()]).unwrap();

        assert!(filter.matches("JP"));
        assert!(filter.matches("jp"));
        assert!(filter.matches("KR"));
        assert!(!filter.matches("US"));
    }

    #[test]
    fn empty_filter_matches_everything() {
        let filter = CountryFilter::new();
        assert!(filter.is_empty());
        assert!(filter.matches("JP"));
        assert!(filter.matches("ZZ"));
        assert!(filter.matches(""));
    }

    #[test]
    fn repeated_and_comma_values_combine() {
        let filter = CountryFilter::from_args(&["jp,kr".to_string(), " us ".to_string()]).unwrap();
        assert!(filter.matches("JP"));
        assert!(filter.matches("KR"));
        assert!(filter.matches("US"));
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(CountryFilter::from_args(&["JPX".to_string()]).is_err());
    }

    #[test]
    fn ignores_empty_segments() {
        let filter = CountryFilter::from_args(&["jp,,kr".to_string()]).unwrap();
        assert!(filter.matches("JP"));
        assert!(filter.matches("KR"));
    }

    #[test]
    fn display_is_sorted() {
        let filter = CountryFilter::from_args(&["kr,jp".to_string()]).unwrap();
        assert_eq!(filter.to_display(), "JP, KR");
        assert_eq!(CountryFilter::new().to_display(), "ALL");
    }
}

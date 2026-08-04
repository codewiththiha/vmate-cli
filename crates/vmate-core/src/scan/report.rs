//! Scan options, progress reporting and the final report.

use crate::country::CountryCode;
use crate::filter::CountryFilter;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A successful config that matched the country filter.
#[derive(Debug, Clone)]
pub struct ScanMatch {
    pub path: PathBuf,
    pub country: CountryCode,
}

/// Aggregated results of a scan.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub scanned: usize,
    pub tested: usize,
    pub success: usize,
    pub matched: usize,
    pub matched_configs: Vec<ScanMatch>,
    pub saved_to_db: bool,
    pub filter: String,
}

/// Everything a scan needs to know.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub dir: PathBuf,
    /// Max number of *filtered matches* to collect.
    pub limit: usize,
    pub timeout: Duration,
    pub workers: usize,
    pub modify: bool,
    pub backup: bool,
    pub no_save: bool,
    pub filter: CountryFilter,
}

/// Receives progress updates from the scan service.
///
/// Implementations are shared between worker tasks, so all methods take `&self`
/// and must be internally synchronized.
pub trait ScanProgress: Send + Sync {
    fn total(&self, total: usize);
    /// One config finished testing.
    fn tested(&self);
    /// One config succeeded.
    fn ok(&self);
    /// One config matched the filter.
    fn matched(&self);
    fn success(&self, path: &Path, country: &CountryCode);
    fn failed(&self, path: &Path);
    /// Called when the scan finishes (lets a progress bar clear itself).
    fn finish(&self) {}
}

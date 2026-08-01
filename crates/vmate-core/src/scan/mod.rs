//! Scan orchestration: concurrent testing, storage and reporting.

pub mod report;
pub mod service;

pub use report::{ScanMatch, ScanOptions, ScanProgress, ScanReport};
pub use service::ScanService;

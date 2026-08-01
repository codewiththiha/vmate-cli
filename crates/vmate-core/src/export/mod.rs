//! Export successful configs to a directory with sanitized names.

pub mod service;

pub use service::{ExportResult, export_configs, sanitize_filename, unique_destination};

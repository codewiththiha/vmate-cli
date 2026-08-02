//! Export successful configs to a directory with sanitized names.

pub mod service;

pub use service::{
    ExportResult, export_configs, export_configs_from_matches, sanitize_filename,
    unique_destination,
};

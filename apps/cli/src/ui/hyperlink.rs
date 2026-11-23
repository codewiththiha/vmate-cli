//! OSC 8 terminal hyperlinks.

/// Wrap `path` in an OSC 8 hyperlink to the file.
///
/// Returns `None` when the path cannot be converted to a file URL.
pub fn osc8_file_hyperlink(path: &str) -> Option<String> {
    let url = url::Url::from_file_path(path).ok()?;
    Some(format!("\x1b]8;;{url}\x1b\\{path}\x1b]8;;\x1b\\"))
}

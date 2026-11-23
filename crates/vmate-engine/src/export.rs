//! Bounded desktop exports with collision-safe, non-overwriting filenames.

use crate::{Port, model::ExportItem};
use anyhow::{Context, Result, ensure};
use std::path::Path;

/// Build the same safe filename for dialog previews and actual exports.
pub fn filename(name: &str, country: &str, style: i32) -> String {
    let stem = name.strip_suffix(".ovpn").unwrap_or(name);
    let label = match style {
        1 => country.to_string(),
        2 => format!("{stem}_{country}"),
        _ => stem.to_string(),
    };
    let safe = vmate_core::export::sanitize_filename(&label, "");
    let safe = safe.strip_prefix('_').unwrap_or(&safe);
    let safe: String = safe
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .take(160)
        .collect();
    format!(
        "{}.ovpn",
        if safe.trim_matches('.').is_empty() {
            "relay"
        } else {
            &safe
        }
    )
}

pub(super) fn batch(port: &Port, items: Vec<ExportItem>, directory: &Path) -> Result<()> {
    ensure!(
        !items.is_empty() && items.len() <= 500,
        "Select 1–500 relays"
    );
    ensure!(directory.is_dir(), "Choose an existing export folder");
    let snapshot = port.snapshot();
    for item in &items {
        ensure!(
            snapshot.relays.iter().any(|r| r.path == item.path),
            "Relay is no longer in history"
        );
        ensure!(
            Path::new(&item.filename)
                .file_name()
                .and_then(|s| s.to_str())
                == Some(&item.filename)
                && item.filename.ends_with(".ovpn"),
            "Invalid export filename"
        );
    }
    let total = items.len();
    for (index, item) in items.into_iter().enumerate() {
        let stem = item
            .filename
            .strip_suffix(".ovpn")
            .context("Missing config extension")?;
        let mut written = false;
        for suffix in 0..10_000 {
            let name = if suffix == 0 {
                item.filename.clone()
            } else {
                format!("{stem}_{suffix}.ovpn")
            };
            let destination = directory.join(name);
            match copy_new(&item.path, &destination) {
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Export stopped after {index}/{total} files"));
                }
                Ok(()) => {
                    written = true;
                    break;
                }
            }
        }
        ensure!(
            written,
            "Export stopped after {index}/{total} files: too many filename collisions"
        );
    }
    let mut state = port.state.lock().unwrap();
    state.snapshot.message = format!("Exported {total} relays to {}", directory.display());
    let message = state.snapshot.message.clone();
    state.event(message);
    Ok(())
}

pub(super) fn copy_new(source: &Path, destination: &Path) -> std::io::Result<()> {
    let mut source = std::fs::File::open(source)?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    if let Err(error) = std::io::copy(&mut source, &mut target) {
        drop(target);
        let _ = std::fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}

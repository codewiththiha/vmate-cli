//! Export implementation.

use crate::db::ConfigRepo;
use crate::filter::CountryFilter;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Outcome of an export run.
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub exported: usize,
    pub total: usize,
    pub dest: PathBuf,
}

/// Build a country-prefixed, filesystem-safe file name.
///
/// Spaces become underscores, path separators and other reserved characters
/// are replaced, and the country is prepended: `JP_my config.ovpn` becomes
/// `JP_my_config.ovpn`.
pub fn sanitize_filename(name: &str, country: &str) -> String {
    let mut s = name.trim().to_string();
    s = s.replace(' ', "_");
    for ch in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        s = s.replace(ch, "_");
    }
    if s.is_empty() {
        s = "config.ovpn".to_string();
    }
    format!("{country}_{s}")
}

/// Return a destination path that does not collide with an existing file,
/// appending `_1`, `_2`, ... as needed.
pub fn unique_destination(dest_dir: &Path, desired: &str) -> PathBuf {
    let candidate = dest_dir.join(desired);
    if !candidate.exists() {
        return candidate;
    }

    let (Some(stem), Some(ext)) = (
        Path::new(desired).file_stem().and_then(|s| s.to_str()),
        Path::new(desired).extension().and_then(|s| s.to_str()),
    ) else {
        return candidate;
    };

    for i in 1.. {
        let alt = dest_dir.join(format!("{stem}_{i}.{ext}"));
        if !alt.exists() {
            return alt;
        }
    }
    candidate
}

/// Copy successful configs matching `filter` into `dest`.
pub async fn export_configs(
    repo: &ConfigRepo,
    filter: &CountryFilter,
    dest: &Path,
) -> Result<ExportResult> {
    let all = repo.all_successful().await?;
    let matched: Vec<_> = all.iter().filter(|c| filter.matches(&c.country)).collect();

    std::fs::create_dir_all(dest)
        .with_context(|| format!("cannot create export directory {}", dest.display()))?;

    let mut exported = 0;
    for config in &matched {
        let src = Path::new(&config.path);
        let name = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("config.ovpn");
        let desired = sanitize_filename(name, &config.country);
        let dst = unique_destination(dest, &desired);
        match std::fs::copy(src, &dst) {
            Ok(_) => exported += 1,
            Err(e) => tracing::warn!(src = %src.display(), error = %e, "export copy failed"),
        }
    }

    Ok(ExportResult {
        exported,
        total: matched.len(),
        dest: dest.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(
            sanitize_filename("my config.ovpn", "JP"),
            "JP_my_config.ovpn"
        );
        assert_eq!(sanitize_filename("a/b\\c:*.ovpn", "KR"), "KR_a_b_c__.ovpn");
        assert_eq!(sanitize_filename("config", "JP"), "JP_config");
        assert_eq!(sanitize_filename("", "JP"), "JP_config.ovpn");
    }

    #[test]
    fn unique_destination_appends_counters() {
        let dir = tempfile::tempdir().unwrap();
        let a = unique_destination(dir.path(), "JP_a.ovpn");
        assert_eq!(a, dir.path().join("JP_a.ovpn"));

        std::fs::write(&a, "x").unwrap();
        let b = unique_destination(dir.path(), "JP_a.ovpn");
        assert_eq!(b, dir.path().join("JP_a_1.ovpn"));

        std::fs::write(&b, "x").unwrap();
        let c = unique_destination(dir.path(), "JP_a.ovpn");
        assert_eq!(c, dir.path().join("JP_a_2.ovpn"));
    }
}

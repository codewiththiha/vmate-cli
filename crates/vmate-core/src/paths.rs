//! Path helpers: `~` expansion and default config paths.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

/// Expand a leading `~` to the current user's home directory.
pub fn expand_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let raw = path.as_ref().as_os_str().to_string_lossy();
    if raw == "~" || raw.starts_with("~/") {
        let expanded = shellexpand::tilde(&raw);
        PathBuf::from(expanded.as_ref())
    } else {
        path.as_ref().to_path_buf()
    }
}

/// Expand `~`, then canonicalize the path.
///
/// Errors with a friendly message when the directory does not exist.
pub fn canonicalize_dir<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let expanded = expand_path(path);
    std::fs::canonicalize(&expanded)
        .with_context(|| format!("directory does not exist: {}", expanded.display()))
}

/// The user configuration directory (`~/.config/vmate-cli` on Unix).
pub fn config_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "vmate-cli")
        .ok_or_else(|| anyhow!("cannot determine config directory"))?;
    Ok(dirs.config_dir().to_path_buf())
}

/// The default SQLite database path, creating the config directory if needed.
pub fn default_db_path() -> Result<PathBuf> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("cannot create config directory {}", dir.display()))?;
    Ok(dir.join("vmate.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_prefix() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let expanded = expand_path("~/configs");
        assert_eq!(expanded, PathBuf::from(format!("{home}/configs")));
        assert_eq!(expand_path("~"), PathBuf::from(&home));
    }

    #[test]
    fn leaves_plain_paths_alone() {
        assert_eq!(expand_path("/etc/openvpn"), PathBuf::from("/etc/openvpn"));
        assert_eq!(expand_path("relative/path"), PathBuf::from("relative/path"));
    }
}

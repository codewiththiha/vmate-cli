//! Path helpers: `~` expansion, default config paths and storage ownership.

use anyhow::{Context, Result, anyhow, bail};
use nix::unistd::{Uid, User, getgid, getuid};
use std::os::unix::fs::MetadataExt;
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
///
/// Under `sudo` the *invoking* user's directory is returned, so an elevated run
/// and a normal run share one database. Linux `sudo` resets `HOME` to `/root`
/// (`always_set_home`) while macOS `sudo` keeps it, which is exactly why the
/// directory is derived from `SUDO_UID` instead of from the environment.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = sudo_user_config_dir() {
        return Ok(dir);
    }
    let dirs = directories::ProjectDirs::from("", "", "vmate-cli")
        .ok_or_else(|| anyhow!("cannot determine config directory"))?;
    Ok(dirs.config_dir().to_path_buf())
}

/// The directory that materialized built-in configs live under.
pub fn builtin_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("builtin"))
}

/// The default SQLite database path, creating the config directory if needed.
pub fn default_db_path() -> Result<PathBuf> {
    let dir = config_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create config directory {}", dir.display()))?;
    }
    // A directory created by an elevated run belongs to root: hand it back so
    // the next unprivileged run can still write to it.
    crate::system::repair_ownership(&dir);

    let db = dir.join("vmate.db");
    ensure_writable(&db, &dir)?;
    Ok(db)
}

/// Refuse early — and with a fix — when the database cannot be written to.
///
/// The usual cause is an older elevated run: on Linux `sudo` resets `HOME`, so
/// vmate stored its history under `/root` and, when the directory was shared,
/// left root-owned files behind that the normal user cannot open.
fn ensure_writable(db: &Path, dir: &Path) -> Result<()> {
    let target = if db.exists() { db } else { dir };
    if writable_by_current_user(target) {
        return Ok(());
    }
    let owner = owner_uid(target)
        .map(|uid| uid.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    bail!(
        "cannot write to {} (owned by uid {owner})\n\
         hint: it was created by an elevated run — hand it back with\n\
         \x20 `sudo chown -R \"$(id -u):$(id -g)\" {}`",
        target.display(),
        dir.display()
    );
}

/// Whether `path` can be written by the current (real) user.
///
/// Root bypasses the check: it can write regardless of the mode bits.
pub fn writable_by_current_user(path: &Path) -> bool {
    if crate::system::is_root() {
        return true;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        // Missing: whether it can be created is decided by the parent.
        return path.parent().is_some_and(writable_by_current_user);
    };

    // A directory needs write *and* search (x) permission to be usable.
    let (owner_bits, group_bits, other_bits) = if meta.is_dir() {
        (0o300, 0o030, 0o003)
    } else {
        (0o200, 0o020, 0o002)
    };
    let required = if meta.uid() == getuid().as_raw() {
        owner_bits
    } else if meta.gid() == getgid().as_raw() {
        group_bits
    } else {
        other_bits
    };
    (meta.mode() & required) == required
}

/// The owning uid of `path`, when its metadata can be read.
pub fn owner_uid(path: &Path) -> Option<u32> {
    std::fs::metadata(path).ok().map(|meta| meta.uid())
}

/// The invoking user's config directory when this process is the elevated half
/// of a `sudo` run.
fn sudo_user_config_dir() -> Option<PathBuf> {
    let (uid, _gid) = crate::system::sudo_uid_gid()?;
    let user = User::from_uid(Uid::from_raw(uid)).ok()??;
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => user.dir.join(".config"),
    };
    Some(base.join("vmate-cli"))
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

    #[test]
    fn config_dir_is_absolute_and_named_vmate_cli() {
        let dir = config_dir().expect("config dir");
        assert!(dir.is_absolute(), "config dir must be absolute: {dir:?}");
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("vmate-cli"));
    }

    #[test]
    fn the_config_dir_itself_is_writable() {
        let dir = config_dir().expect("config dir");
        assert!(
            writable_by_current_user(&dir),
            "the config dir must be writable: {dir:?}"
        );
    }

    #[test]
    fn missing_paths_fall_back_to_their_parent() {
        let dir = std::env::temp_dir().join("vmate-missing-parent-probe");
        assert!(writable_by_current_user(&dir));
        if !crate::system::is_root() {
            assert!(!writable_by_current_user(Path::new(
                "/proc/definitely-missing-vmate.db"
            )));
        }
    }
}

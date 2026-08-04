//! Persistent user defaults for scan/connect tunables.
//!
//! Tunables such as scan `--max`/`--limit`/`--timeout` and connect
//! `--connect-timeout`/`--cooldown`/`--retry-count`/stability grace are
//! normally fixed to built-in defaults. Users can override them per-run with
//! the value flags, or persist them across sessions by passing `--save-defaults`
//! along with the explicitly-passed flags.
//!
//! The effective value for a tunable is resolved as:
//!
//! `explicit CLI flag → persisted setting → built-in default`
//!
//! Persisted settings live in `~/.config/vmate-cli/settings.json` and are never
//! allowed to break the CLI: a missing or corrupt file simply resolves to the
//! built-in defaults.

use crate::paths;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// User-chosen defaults for scan/connect tunables.
///
/// `None` fields mean "not persisted" and fall back to the built-in default.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserSettings {
    /// scan `--max` (concurrent test processes) default.
    pub max_workers: Option<u64>,
    /// scan `--limit` default.
    pub limit: Option<u64>,
    /// scan `--timeout` (seconds) default.
    pub scan_timeout_secs: Option<u64>,
    /// connect `--connect-timeout` (seconds) default.
    pub connect_timeout_secs: Option<u64>,
    /// connect `--cooldown` (seconds) default.
    pub cooldown_secs: Option<u64>,
    /// connect retry count default.
    pub retry_count: Option<u32>,
    /// connect stability grace (seconds) default.
    pub stability_grace_secs: Option<u64>,
}

impl UserSettings {
    /// Load settings from the default config path.
    ///
    /// A missing or corrupt file resolves to `Default` (never fails).
    pub fn load() -> Self {
        match Self::path() {
            Ok(p) => Self::load_from(&p),
            Err(_) => Self::default(),
        }
    }

    /// Write settings to the default config path, creating the directory.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path()?;
        self.save_to(&path)
    }

    /// The default settings file path.
    pub fn path() -> anyhow::Result<PathBuf> {
        Ok(paths::config_dir()?.join("settings.json"))
    }

    /// Load settings from an explicit path.
    ///
    /// On any IO or parse error (including a missing file) returns
    /// [`UserSettings::default`] — a broken settings file must never break the
    /// CLI.
    pub fn load_from(path: &Path) -> Self {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    /// Save settings to an explicit path, creating parent directories.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Resolve the scan `--max` value: explicit flag OR persisted OR 64.
    pub fn max_workers(&self, explicit: Option<u64>) -> u64 {
        explicit.or(self.max_workers).unwrap_or(64)
    }

    /// Resolve the scan `--limit` value: explicit flag OR persisted OR 100.
    pub fn limit(&self, explicit: Option<u64>) -> u64 {
        explicit.or(self.limit).unwrap_or(100)
    }

    /// Resolve the scan `--timeout` value: explicit flag OR persisted OR 15s.
    pub fn scan_timeout(&self, explicit: Option<Duration>) -> Duration {
        explicit
            .or(self.scan_timeout_secs.map(Duration::from_secs))
            .unwrap_or(Duration::from_secs(15))
    }

    /// Resolve the connect `--connect-timeout` value: explicit OR persisted OR 5s.
    pub fn connect_timeout(&self, explicit: Option<Duration>) -> Duration {
        explicit
            .or(self.connect_timeout_secs.map(Duration::from_secs))
            .unwrap_or(Duration::from_secs(5))
    }

    /// Resolve the connect `--cooldown` value: explicit OR persisted OR 30s.
    pub fn cooldown(&self, explicit: Option<Duration>) -> Duration {
        explicit
            .or(self.cooldown_secs.map(Duration::from_secs))
            .unwrap_or(Duration::from_secs(30))
    }

    /// Resolve the connect retry count: explicit OR persisted OR 2.
    pub fn retry_count(&self, explicit: Option<u32>) -> u32 {
        explicit.or(self.retry_count).unwrap_or(2)
    }

    /// Resolve the connect stability grace: explicit OR persisted OR 5s.
    pub fn stability_grace(&self, explicit: Option<Duration>) -> Duration {
        explicit
            .or(self.stability_grace_secs.map(Duration::from_secs))
            .unwrap_or(Duration::from_secs(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample() -> UserSettings {
        UserSettings {
            max_workers: Some(128),
            limit: Some(50),
            scan_timeout_secs: Some(20),
            connect_timeout_secs: Some(7),
            cooldown_secs: Some(45),
            retry_count: Some(3),
            stability_grace_secs: Some(9),
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        let original = sample();

        original.save_to(&path).unwrap();
        let loaded = UserSettings::load_from(&path);

        assert_eq!(loaded.max_workers, original.max_workers);
        assert_eq!(loaded.limit, original.limit);
        assert_eq!(loaded.scan_timeout_secs, original.scan_timeout_secs);
        assert_eq!(loaded.connect_timeout_secs, original.connect_timeout_secs);
        assert_eq!(loaded.cooldown_secs, original.cooldown_secs);
        assert_eq!(loaded.retry_count, original.retry_count);
        assert_eq!(loaded.stability_grace_secs, original.stability_grace_secs);
    }

    #[test]
    fn missing_file_loads_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let loaded = UserSettings::load_from(&path);
        assert_eq!(loaded, UserSettings::default());
    }

    #[test]
    fn corrupt_json_loads_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not valid json !!!").unwrap();
        let loaded = UserSettings::load_from(&path);
        assert_eq!(loaded, UserSettings::default());
    }

    #[test]
    fn resolvers_use_builtin_default_with_no_explicit_or_persisted() {
        let us = UserSettings::default();
        let s = |n: u64| Duration::from_secs(n);
        assert_eq!(us.max_workers(None), 64);
        assert_eq!(us.limit(None), 100);
        assert_eq!(us.scan_timeout(None), s(15));
        assert_eq!(us.connect_timeout(None), s(5));
        assert_eq!(us.cooldown(None), s(30));
        assert_eq!(us.retry_count(None), 2);
        assert_eq!(us.stability_grace(None), s(5));
    }

    #[test]
    fn resolvers_prefer_persisted_over_builtin() {
        let us = sample();
        let s = |n: u64| Duration::from_secs(n);
        assert_eq!(us.max_workers(None), 128);
        assert_eq!(us.limit(None), 50);
        assert_eq!(us.scan_timeout(None), s(20));
        assert_eq!(us.connect_timeout(None), s(7));
        assert_eq!(us.cooldown(None), s(45));
        assert_eq!(us.retry_count(None), 3);
        assert_eq!(us.stability_grace(None), s(9));
    }

    #[test]
    fn explicit_value_wins_over_persisted() {
        let us = sample();
        let s = |n: u64| Duration::from_secs(n);
        assert_eq!(us.max_workers(Some(256)), 256);
        assert_eq!(us.limit(Some(10)), 10);
        assert_eq!(us.scan_timeout(Some(s(30))), s(30));
        assert_eq!(us.connect_timeout(Some(s(3))), s(3));
        assert_eq!(us.cooldown(Some(s(5))), s(5));
        assert_eq!(us.retry_count(Some(5)), 5);
        assert_eq!(us.stability_grace(Some(s(1))), s(1));
    }
}

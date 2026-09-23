//! Root privilege handling, sudo elevation and ownership repair.

use anyhow::{Result, bail};
use nix::unistd::getuid;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Environment carried across a `sudo` re-exec so the elevated half resolves
/// the same home, config directory, database and settings as the invoking
/// user's run does.
///
/// Linux `sudo` sets `HOME=/root` (`always_set_home`), which used to split
/// vmate's storage in two: an elevated `scan`/`connect` wrote
/// `/root/.config/vmate-cli/vmate.db` while `recent` — which never elevates —
/// read `~/.config/vmate-cli/vmate.db` and reported an empty history. macOS
/// `sudo` keeps `HOME`, so the split only ever showed up on Linux.
const CARRIED_ENV: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TERM",
    "LANG",
    "LC_ALL",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "VMATE_DB",
    "VMATE_OPENVPN_BIN",
    "IPINFO_TOKEN",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
];

/// Marker proving this process is already the elevated half of a re-exec. A
/// sudo policy that leaves us unprivileged must never loop on sudo.
const ELEVATED_MARKER: &str = "VMATE_ELEVATED";

/// Whether the current process runs as uid 0.
pub fn is_root() -> bool {
    getuid().is_root()
}

/// Whether stdin and stdout are both terminals (i.e. prompting is possible).
pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// The user who invoked `sudo`, when this process is the elevated half.
pub fn sudo_user() -> Option<String> {
    std::env::var("SUDO_USER").ok().filter(|s| !s.is_empty())
}

/// The invoking user's uid/gid, when this process is the elevated half.
pub fn sudo_uid_gid() -> Option<(u32, u32)> {
    if !is_root() {
        return None;
    }
    let uid = std::env::var("SUDO_UID").ok()?.parse::<u32>().ok()?;
    let gid = std::env::var("SUDO_GID").ok()?.parse::<u32>().ok()?;
    Some((uid, gid))
}

/// Whether this process is the elevated half of a sudo re-exec.
pub fn is_elevated() -> bool {
    std::env::var_os(ELEVATED_MARKER).is_some()
}

/// Hand a root-created path back to the user who invoked sudo.
///
/// Without this an elevated run leaves `~/.config/vmate-cli` — and the database
/// inside it — owned by root, and the next unprivileged run cannot open it.
/// Best effort by design: a failed chown must never abort a scan.
pub fn repair_ownership(path: &Path) {
    let Some((uid, gid)) = sudo_uid_gid() else {
        return;
    };
    if !path.exists() {
        return;
    }
    #[cfg(unix)]
    if let Err(err) = std::os::unix::fs::chown(path, Some(uid), Some(gid)) {
        tracing::debug!(
            path = %path.display(),
            error = %err,
            "could not hand ownership back to the invoking user"
        );
    }
}

/// `KEY=value` assignments that re-create the user's identity after `sudo`.
pub fn carried_environment() -> Vec<String> {
    let mut assignments: Vec<String> = CARRIED_ENV
        .iter()
        .filter_map(|key| {
            std::env::var_os(*key).map(|value| format!("{key}={}", value.to_string_lossy()))
        })
        .collect();
    assignments.push(format!("{ELEVATED_MARKER}=1"));
    assignments
}

/// Ensure the process has root privileges for `context`.
///
/// * Already root → returns.
/// * `no_elevate` is set or `VMATE_NO_ELEVATE` is set → warns and proceeds
///   (OpenVPN will likely fail; this is the escape hatch used by tests and CI).
/// * Interactive TTY → transparently re-executes under `sudo`.
/// * Otherwise → returns an error explaining how to run elevated.
pub fn require_root_for(context: &str, no_elevate: bool) -> Result<()> {
    if is_root() {
        return Ok(());
    }

    if no_elevate || std::env::var_os("VMATE_NO_ELEVATE").is_some() {
        tracing::warn!("running without root privileges to {context}; OpenVPN will likely fail");
        return Ok(());
    }

    // We already went through sudo once and are somehow still unprivileged:
    // bailing is the only way to avoid re-executing forever.
    if is_elevated() {
        bail!(
            "vmate-cli was elevated with sudo but is still not root, so it cannot {context}\n\
             hint: check `sudo -v`, or run `sudo vmate-cli ...` yourself"
        );
    }

    if !interactive() {
        bail!(
            "vmate-cli needs root to {context}\n\
             hint: run `sudo vmate-cli ...`"
        );
    }

    elevate_with_sudo()
}

/// Re-execute the current binary with `sudo`, preserving arguments.
///
/// This function never returns: the child runs under sudo and this process
/// exits with its exit code.
pub fn elevate_with_sudo() -> ! {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("vmate-cli"));
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    // `sudo env HOME=... <exe> <args...>`: `sudo` resets the environment (and,
    // on Linux, resets HOME to /root), so the user's identity is re-applied by
    // `env` *after* sudo. That works with a stock sudoers file, where `-E`
    // would be refused.
    let status = std::process::Command::new("sudo")
        .arg("env")
        .args(carried_environment())
        .arg(&exe)
        .args(&args)
        .status();

    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("error: failed to elevate with sudo: {e}");
            std::process::exit(1);
        }
    }
}

/// One-line privilege status for `vmate-cli doctor`.
pub fn root_summary() -> String {
    if !is_root() {
        return "no".to_string();
    }
    match sudo_user() {
        Some(user) => format!("yes (elevated via sudo as {user})"),
        None => "yes".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carried_environment_restores_home_and_marks_elevation() {
        // HOME is set in every environment we run in, so the pair must appear.
        let assignments = carried_environment();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        assert!(
            assignments.iter().any(|a| a == &format!("HOME={home}")),
            "HOME must survive sudo: {assignments:?}"
        );
        assert!(
            assignments
                .iter()
                .any(|a| a == &format!("{ELEVATED_MARKER}=1")),
            "the elevated half must be marked: {assignments:?}"
        );
    }

    #[test]
    fn carried_environment_has_no_duplicate_keys() {
        let mut keys: Vec<String> = carried_environment()
            .iter()
            .filter_map(|a| a.split_once('=').map(|(k, _)| k.to_string()))
            .collect();
        let total = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), total, "duplicate assignment: {keys:?}");
    }

    #[test]
    fn repair_ownership_ignores_missing_paths() {
        // A no-op rather than an error: ownership repair is best effort.
        repair_ownership(Path::new("/nonexistent/vmate-ownership-probe"));
    }

    #[test]
    fn unprivileged_processes_have_no_sudo_identity() {
        // Guards the is_root() short circuit: without it a normal run would
        // try to chown user-owned files to a bogus uid.
        if !is_root() {
            assert!(sudo_uid_gid().is_none());
        }
    }
}

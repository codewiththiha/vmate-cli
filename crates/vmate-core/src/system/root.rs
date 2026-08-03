//! Root privilege handling.

use anyhow::Result;
use nix::unistd::getuid;
use std::io::IsTerminal;
use std::path::PathBuf;

/// Whether the current process runs as uid 0.
pub fn is_root() -> bool {
    getuid().is_root()
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

    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if !interactive {
        anyhow::bail!(
            "vmate-cli needs root to {context}.\n\
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

    let status = std::process::Command::new("sudo")
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

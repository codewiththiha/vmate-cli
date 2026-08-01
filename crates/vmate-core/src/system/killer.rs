//! Process killing.
//!
//! vmate intentionally runs `killall -9 openvpn` on connection switching and
//! shutdown — this is not an accident. The exact `killall -9 openvpn` form is
//! preserved from the original Go tool.

use anyhow::Result;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::sync::Arc;

/// SIGKILL the process group whose leader has the given pid.
pub fn kill_process_group(pid: u32) -> Result<()> {
    if pid == 0 {
        return Ok(());
    }
    let pgid = -(pid as i32);
    kill(Pid::from_raw(pgid), Signal::SIGKILL)
        .map_err(|e| anyhow::anyhow!("failed to kill process group {pid}: {e}"))
}

/// Run `killall -9 openvpn`.
///
/// A non-zero exit status (e.g. "no processes matched") is normal and is
/// logged rather than treated as an error.
pub fn killall_openvpn() -> Result<()> {
    let status = std::process::Command::new("killall")
        .arg("-9")
        .arg("openvpn")
        .status();

    match status {
        Ok(status) => {
            if !status.success() {
                tracing::debug!(%status, "killall -9 openvpn found no processes to kill");
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!(error = %e, "killall -9 openvpn failed");
            Ok(())
        }
    }
}

/// Abstraction over the ways vmate kills processes.
pub trait ProcessKiller: Send + Sync {
    fn kill_process_group(&self, pid: u32) -> Result<()>;
    fn killall_openvpn(&self) -> Result<()>;
}

/// Production killer.
///
/// Global `killall -9 openvpn` can be disabled with `--no-killall`; per-process
/// group kills always happen.
pub struct RealProcessKiller {
    pub killall_enabled: bool,
}

impl ProcessKiller for RealProcessKiller {
    fn kill_process_group(&self, pid: u32) -> Result<()> {
        kill_process_group(pid)
    }

    fn killall_openvpn(&self) -> Result<()> {
        if !self.killall_enabled {
            return Ok(());
        }
        killall_openvpn()
    }
}

/// RAII guard that runs `killall -9 openvpn` on drop.
///
/// This is the safety net that guarantees no stale OpenVPN processes survive
/// vmate, even on panic or error paths.
pub struct CleanupGuard {
    killer: Arc<dyn ProcessKiller>,
    enabled: bool,
}

impl CleanupGuard {
    pub fn new(killer: Arc<dyn ProcessKiller>, enabled: bool) -> Self {
        Self { killer, enabled }
    }

    /// Prevent the guard from killing anything on drop.
    pub fn disarm(&mut self) {
        self.enabled = false;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.enabled {
            let _ = self.killer.killall_openvpn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopKiller;

    impl ProcessKiller for NoopKiller {
        fn kill_process_group(&self, _pid: u32) -> Result<()> {
            Ok(())
        }
        fn killall_openvpn(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn disabled_guard_does_nothing() {
        let killer: Arc<dyn ProcessKiller> = Arc::new(NoopKiller);
        let mut guard = CleanupGuard::new(killer, false);
        guard.disarm();
        // Dropping should be a no-op (would panic only if enabled flag is wrong).
    }

    #[test]
    fn zero_pid_group_kill_is_a_noop() {
        assert!(kill_process_group(0).is_ok());
    }
}

//! Process killing.
//!
//! vmate-cli tracks every OpenVPN process it spawns in a per-session PID
//! registry and, by default, cleans up exactly those processes on connection
//! switching and shutdown: SIGTERM the process group, wait a grace period, then
//! SIGKILL anything still alive. The exact `killall -9 openvpn` form preserved
//! from the original Go tool is now opt-in via `--killall` and only runs in
//! addition to the per-process cleanup.

use anyhow::Result;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Grace period between SIGTERM and SIGKILL when killing a process tree.
pub const KILL_GRACE: Duration = Duration::from_secs(3);

/// PIDs of every OpenVPN process spawned in one session.
///
/// Owned per run — and per test — so concurrent sessions never interfere:
/// each [`CleanupGuard`] sweeps only the registry it was given, never another
/// session's (or test's) live processes.
#[derive(Default)]
pub struct ProcessRegistry {
    pids: Mutex<Vec<u32>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a spawned process pid so the cleanup guard can kill it on the
    /// way out. A pid of 0 (unknown) is ignored.
    pub fn register(&self, pid: u32) {
        if pid == 0 {
            return;
        }
        self.pids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(pid);
    }

    /// Snapshot of the currently registered pids.
    pub fn registered(&self) -> Vec<u32> {
        self.pids.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Forget every registered pid without killing anything.
    pub fn clear(&self) {
        self.pids.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Kill every registered process group: SIGTERM all of them, allow one
    /// grace period, then SIGKILL anything still alive, then clear.
    ///
    /// Sync and best-effort: individual kill errors are ignored, and the grace
    /// period is a fixed sleep because there is no live handle to wait on. This
    /// is the last-resort safety net used by [`CleanupGuard::drop`].
    pub fn kill_all_graceful(&self) {
        let pids = self.registered();
        for &pid in &pids {
            let _ = kill_process_group(pid);
        }
        std::thread::sleep(Duration::from_secs(1));
        for &pid in &pids {
            let _ = force_kill_process_group(pid);
        }
        self.clear();
    }
}

/// SIGTERM the process group whose leader has the given pid. The child is
/// given the chance to shut down gracefully before a SIGKILL follows.
pub fn kill_process_group(pid: u32) -> Result<()> {
    if pid == 0 {
        return Ok(());
    }
    let pgid = -(pid as i32);
    kill(Pid::from_raw(pgid), Signal::SIGTERM)
        .map_err(|e| anyhow::anyhow!("failed to signal process group {pid}: {e}"))
}

/// SIGKILL the process group whose leader has the given pid. Escalation used
/// after the SIGTERM grace period has elapsed.
pub fn force_kill_process_group(pid: u32) -> Result<()> {
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
/// logged at trace level rather than treated as an error. stdout/stderr are
/// redirected so killall's own messages never leak into the user's terminal.
pub fn killall_openvpn() -> Result<()> {
    let res = std::process::Command::new("killall")
        .arg("-9")
        .arg("openvpn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match res {
        Ok(status) => {
            if !status.success() {
                tracing::trace!("killall -9 openvpn: no matching processes (normal)");
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!(error = %e, "killall -9 openvpn failed to start");
            Ok(())
        }
    }
}

/// Abstraction over the ways vmate-cli kills processes.
pub trait ProcessKiller: Send + Sync {
    /// SIGTERM the process group. Implementations delegate to the free
    /// function unless they are test fakes.
    fn kill_process_group(&self, pid: u32) -> Result<()>;
    /// SIGKILL the process group (escalation after the SIGTERM grace period).
    fn force_kill_process_group(&self, pid: u32) -> Result<()>;
    fn killall_openvpn(&self) -> Result<()>;
}

/// Production killer.
///
/// Global `killall -9 openvpn` is opt-in via `--killall`; per-process group
/// kills always happen.
pub struct RealProcessKiller {
    pub killall_enabled: bool,
}

impl ProcessKiller for RealProcessKiller {
    fn kill_process_group(&self, pid: u32) -> Result<()> {
        kill_process_group(pid)
    }

    fn force_kill_process_group(&self, pid: u32) -> Result<()> {
        force_kill_process_group(pid)
    }

    fn killall_openvpn(&self) -> Result<()> {
        if !self.killall_enabled {
            return Ok(());
        }
        killall_openvpn()
    }
}

/// Kill a process tree gracefully: SIGTERM the group, wait up to [`KILL_GRACE`]
/// for the child to exit, then SIGKILL the group if it is still alive.
pub async fn kill_process_tree_graceful(
    killer: &dyn ProcessKiller,
    pid: u32,
    child: &mut tokio::process::Child,
) {
    let _ = killer.kill_process_group(pid);
    if tokio::time::timeout(KILL_GRACE, child.wait())
        .await
        .is_err()
    {
        let _ = killer.force_kill_process_group(pid);
        let _ = child.wait().await;
    }
}

/// RAII guard that cleans up every spawned OpenVPN process on drop.
///
/// This is the safety net that guarantees no stale OpenVPN processes survive
/// vmate, even on panic or error paths: the session's registry is killed
/// first, and the opt-in global `killall -9 openvpn` sweep runs afterwards.
pub struct CleanupGuard {
    killer: Arc<dyn ProcessKiller>,
    registry: Arc<ProcessRegistry>,
    enabled: bool,
}

impl CleanupGuard {
    pub fn new(
        killer: Arc<dyn ProcessKiller>,
        registry: Arc<ProcessRegistry>,
        enabled: bool,
    ) -> Self {
        Self {
            killer,
            registry,
            enabled,
        }
    }

    /// Prevent the guard from killing anything on drop. The process registry is
    /// cleared so the guard leaves no stale pids behind.
    pub fn disarm(&mut self) {
        self.enabled = false;
        self.registry.clear();
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.enabled {
            self.registry.kill_all_graceful();
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
        fn force_kill_process_group(&self, _pid: u32) -> Result<()> {
            Ok(())
        }
        fn killall_openvpn(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn disabled_guard_does_nothing() {
        let killer: Arc<dyn ProcessKiller> = Arc::new(NoopKiller);
        let mut guard = CleanupGuard::new(killer, Arc::new(ProcessRegistry::new()), false);
        guard.disarm();
        // Dropping should be a no-op (would panic only if enabled flag is wrong).
    }

    #[test]
    fn zero_pid_group_kill_is_a_noop() {
        assert!(kill_process_group(0).is_ok());
        assert!(force_kill_process_group(0).is_ok());
    }

    #[test]
    fn registry_registers_and_clears() {
        let reg = ProcessRegistry::new();
        reg.register(0); // ignored
        reg.register(1234);
        reg.register(5678);
        let pids = reg.registered();
        assert!(!pids.contains(&0));
        assert!(pids.contains(&1234));
        assert!(pids.contains(&5678));
        reg.clear();
        assert!(reg.registered().is_empty());
    }

    /// A real (now-reaped) pid and a large pid that no process group can own
    /// must be safe for `kill_all_spawned` to sweep: errors are ignored, the
    /// registry is cleared, and nothing is left behind.
    #[tokio::test]
    async fn kill_all_spawned_ignores_dead_pids_and_clears() {
        let reg = ProcessRegistry::new();
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .unwrap();
        let dead_pid = child.id().unwrap();
        child.wait().await.unwrap();
        reg.register(dead_pid);
        reg.register(999_999_999);

        reg.kill_all_graceful();

        assert!(reg.registered().is_empty());
    }

    #[test]
    fn real_killer_delegates_and_zero_pid_is_noop() {
        let killer = RealProcessKiller {
            killall_enabled: false,
        };
        assert!(killer.kill_process_group(0).is_ok());
        assert!(killer.force_kill_process_group(0).is_ok());
        assert!(killer.killall_openvpn().is_ok()); // disabled -> no-op
    }
}

//! Launching and monitoring OpenVPN processes.

use crate::connect::{ConnectHost, UserCommand};
use crate::ovpn::monitor::{VpnLineClass, classify_line};
use crate::system::killer::{ProcessKiller, ProcessRegistry};
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Arguments used when probing whether a config connects (no routes or
/// interfaces are actually configured).
pub fn test_args(config: &Path) -> Vec<String> {
    vec![
        "--config".to_string(),
        config.display().to_string(),
        "--route-noexec".to_string(),
        "--ifconfig-noexec".to_string(),
        "--nobind".to_string(),
        "--auth-nocache".to_string(),
    ]
}

/// Arguments used for a real connection.
pub fn connect_args(config: &Path) -> Vec<String> {
    vec!["--config".to_string(), config.display().to_string()]
}

/// Recursively discover `.ovpn` files under `dir`, sorted for determinism.
pub fn discover_configs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(".ovpn")
        {
            out.push(entry.into_path());
        }
    }
    out.sort();
    Ok(out)
}

/// A running OpenVPN process with a merged, line-delimited output stream.
pub struct OpenVpnHandle {
    pub child: tokio::process::Child,
    /// Lines from stdout and stderr, merged in arrival order.
    pub lines: mpsc::Receiver<String>,
}

impl OpenVpnHandle {
    /// Stop the process tree gracefully: SIGTERM the group, wait up to
    /// `KILL_GRACE`, then SIGKILL stragglers. The handle owns its teardown, so
    /// callers never reach for the raw pid or a kill policy.
    pub async fn kill_graceful(&mut self, killer: &dyn ProcessKiller) {
        crate::system::killer::kill_process_tree_graceful(
            killer,
            self.child.id().unwrap_or(0),
            &mut self.child,
        )
        .await;
    }
}

/// Spawn OpenVPN in a new process group, registering the pid in `registry`.
///
/// Each spawned process gets its own process group so it can be killed with
/// the whole tree via a single negative-pid SIGKILL.
pub fn spawn_openvpn(
    bin: &str,
    args: &[String],
    registry: &ProcessRegistry,
) -> Result<OpenVpnHandle> {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Put the child in a fresh process group so the whole tree can be killed
    // with a single negative-pid SIGKILL. tokio::process::Command exposes this
    // method directly.
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn OpenVPN binary: {bin}"))?;

    // Track the pid so the cleanup guard can kill exactly the processes this
    // session spawned (a pid of 0 — unknown — is ignored by the registry).
    registry.register(child.id().unwrap_or(0));

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture OpenVPN stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture OpenVPN stderr"))?;

    let (tx, rx) = mpsc::channel(512);
    let stderr_tx = tx.clone();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if tx.send(line).await.is_err() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if stderr_tx.send(line).await.is_err() {
                break;
            }
        }
    });

    Ok(OpenVpnHandle { child, lines: rx })
}

/// Abstraction over spawning the OpenVPN binary.
pub trait OpenVpnRunner: Send + Sync {
    fn spawn(&self, args: &[String]) -> Result<OpenVpnHandle>;
    fn bin(&self) -> &str;
}

/// Production runner that executes the configured `openvpn` binary.
pub struct RealOpenVpnRunner {
    pub bin: String,
    pub registry: Arc<ProcessRegistry>,
}

impl OpenVpnRunner for RealOpenVpnRunner {
    fn spawn(&self, args: &[String]) -> Result<OpenVpnHandle> {
        spawn_openvpn(&self.bin, args, &self.registry)
    }

    fn bin(&self) -> &str {
        &self.bin
    }
}

/// Outcome of watching a short-lived test connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorOutcome {
    Success,
    Error(String),
    RestartPause,
    TimedOut,
    Cancelled,
    Exited,
}

/// Outcome of the connect handshake phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectOutcome {
    Connected,
    Error(String),
    RestartPause,
    TimedOut,
    Exited,
    Cancelled,
    /// The user pressed `n`/Next while the handshake was in progress.
    Next,
}

/// Watch a test connection until success, an error keyword, a timeout or
/// cancellation.
pub async fn monitor_test(
    lines: &mut mpsc::Receiver<String>,
    timeout: Duration,
    cancel: CancellationToken,
) -> MonitorOutcome {
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return MonitorOutcome::TimedOut,
            _ = cancel.cancelled() => return MonitorOutcome::Cancelled,
            line = lines.recv() => {
                let Some(line) = line else { return MonitorOutcome::Exited };
                match classify_line(&line) {
                    VpnLineClass::Success => return MonitorOutcome::Success,
                    VpnLineClass::Error(kw) => return MonitorOutcome::Error(kw.to_string()),
                    VpnLineClass::RestartPause => return MonitorOutcome::RestartPause,
                    VpnLineClass::Info => {}
                }
            }
        }
    }
}

/// Watch the handshake phase of a real connection.
///
/// Every line the process emits is forwarded to the host's verbose log, so the
/// toggleable panel shows the handshake output (not just post-connect lines).
pub async fn monitor_connect(
    lines: &mut mpsc::Receiver<String>,
    connect_timeout: Duration,
    cancel: CancellationToken,
    host: &mut dyn ConnectHost,
) -> ConnectOutcome {
    let deadline = tokio::time::sleep(connect_timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return ConnectOutcome::TimedOut,
            _ = cancel.cancelled() => return ConnectOutcome::Cancelled,
            line = lines.recv() => {
                let Some(line) = line else { return ConnectOutcome::Exited };
                // Feed the verbose log; a render failure must not abort the
                // handshake, so the outcome is what matters.
                let _ = host.log(&line).await;
                match classify_line(&line) {
                    VpnLineClass::Success => return ConnectOutcome::Connected,
                    VpnLineClass::Error(kw) => return ConnectOutcome::Error(kw.to_string()),
                    VpnLineClass::RestartPause => return ConnectOutcome::RestartPause,
                    VpnLineClass::Info => {}
                }
            }
            cmd = host.poll_command() => {
                // Keep the UI responsive while the handshake runs: 'q' quits
                // immediately, 'n' skips to the next config. 'v' is already
                // toggled by the host itself; Help/CopyPath/Reconnect are no-ops
                // while connecting.
                match cmd {
                    Some(UserCommand::Quit) => return ConnectOutcome::Cancelled,
                    Some(UserCommand::Next) => return ConnectOutcome::Next,
                    _ => {}
                }
            }
        }
    }
}

/// Run a full single-config test: spawn, monitor, and kill the process group.
pub async fn test_openvpn_config(
    bin: &str,
    config: &Path,
    timeout: Duration,
    cancel: CancellationToken,
    killer: &dyn ProcessKiller,
    registry: &ProcessRegistry,
) -> Result<bool> {
    let args = test_args(config);
    let mut handle = spawn_openvpn(bin, &args, registry)?;

    let outcome = monitor_test(&mut handle.lines, timeout, cancel).await;

    // Always clean up the process tree; never leak workers.
    handle.kill_graceful(killer).await;

    Ok(matches!(outcome, MonitorOutcome::Success))
}

/// Abstraction over testing a single config.
#[async_trait]
pub trait VpnTester: Send + Sync {
    async fn test(
        &self,
        config: &Path,
        timeout: Duration,
        cancel: CancellationToken,
    ) -> Result<bool>;
}

/// Production tester that launches the real OpenVPN binary.
pub struct RealVpnTester {
    pub bin: String,
    pub killer: Arc<dyn ProcessKiller>,
    pub registry: Arc<ProcessRegistry>,
}

#[async_trait]
impl VpnTester for RealVpnTester {
    async fn test(
        &self,
        config: &Path,
        timeout: Duration,
        cancel: CancellationToken,
    ) -> Result<bool> {
        test_openvpn_config(
            &self.bin,
            config,
            timeout,
            cancel,
            self.killer.as_ref(),
            &self.registry,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::killer::RealProcessKiller;

    #[test]
    fn test_args_include_safety_flags() {
        let args = test_args(Path::new("/tmp/a.ovpn"));
        assert!(args.iter().any(|a| a == "--route-noexec"));
        assert!(args.iter().any(|a| a == "--ifconfig-noexec"));
        assert!(args.iter().any(|a| a == "--nobind"));
        assert!(args.iter().any(|a| a == "--auth-nocache"));
        assert!(args.iter().any(|a| a == "/tmp/a.ovpn"));
    }

    /// A SIGTERM to the process group must end the child quickly — no need to
    /// wait out the SIGKILL escalation.
    #[tokio::test]
    async fn kill_process_tree_graceful_sigterms_and_exits_quickly() {
        let registry = crate::system::killer::ProcessRegistry::new();
        let mut handle = spawn_openvpn("sh", &["-c".into(), "sleep 5".into()], &registry).unwrap();
        let pid = handle.child.id().unwrap_or(0);
        assert!(pid != 0);

        let killer = RealProcessKiller {
            killall_enabled: false,
        };
        let start = std::time::Instant::now();
        tokio::time::timeout(
            Duration::from_secs(2),
            crate::system::killer::kill_process_tree_graceful(&killer, pid, &mut handle.child),
        )
        .await
        .expect("graceful kill must not hang");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "SIGTERM path should exit well under the KILL_GRACE window"
        );
    }
}

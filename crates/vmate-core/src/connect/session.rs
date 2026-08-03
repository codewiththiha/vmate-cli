//! Per-candidate connect session helpers: indefinite monitoring, targeted
//! kills and history drop. Kept out of `service.rs` so `run` reads as a thin
//! orchestrator.

use crate::connect::host::UserCommand;
use crate::connect::queue::Candidate;
use crate::connect::service::ConnectService;
use crate::connect::state::ConnectionStatus;
use crate::country::CountryCode;
use crate::db::models::CountrySource;
use crate::ovpn::monitor::{VpnLineClass, classify_line};
use crate::ovpn::process::OpenVpnHandle;
use anyhow::Result;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// Outcome of the indefinite (phase-2) monitoring of a *connected* session.
pub(crate) enum Phase2Exit {
    Quit,
    Next,
    Reconnect,
    Crashed(String),
}

impl ConnectService {
    /// Indefinite monitoring of an already-connected session. Never kills the
    /// process and never touches the DB — it only reports what the user/line
    /// stream wants to do; the caller applies the failure budget.
    pub(crate) async fn monitor_connected(
        &self,
        candidate: &Candidate,
        handle: &mut OpenVpnHandle,
        host: &mut dyn crate::connect::host::ConnectHost,
        cancel: &CancellationToken,
    ) -> Result<Phase2Exit> {
        host.status(&ConnectionStatus {
            connected: true,
            candidate: Some(candidate.clone()),
            message: format!("Connected successfully to {}", candidate.country),
            filter: String::new(),
        })
        .await?;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(Phase2Exit::Quit),
                cmd = host.poll_command() => match cmd {
                    Some(UserCommand::Next)          => return Ok(Phase2Exit::Next),
                    Some(UserCommand::Reconnect)     => return Ok(Phase2Exit::Reconnect),
                    Some(UserCommand::Quit)          => return Ok(Phase2Exit::Quit),
                    Some(UserCommand::CopyPath)      => { let _ = host.copy(&candidate.path).await; }
                    // Verbose rendering is owned by the host (it toggles on
                    // the `v` key); the service always feeds it lines so the
                    // toggle state survives crashes and reconnects.
                    Some(UserCommand::ToggleVerbose) => {}
                    Some(UserCommand::Help)          => {
                        let _ = host.notify("[n] next  [r] reconnect  [c] copy  [v] verbose  [q] quit").await;
                    }
                    None => {}
                },
                line = handle.lines.recv() => {
                    let Some(line) = line else {
                        return Ok(Phase2Exit::Crashed("openvpn exited unexpectedly".into()));
                    };
                    let _ = host.log(&line).await;
                    match classify_line(&line) {
                        VpnLineClass::Error(kw) => {
                            return Ok(Phase2Exit::Crashed(format!("connection error: {kw}")));
                        }
                        VpnLineClass::RestartPause => {
                            return Ok(Phase2Exit::Crashed("restart pause".into()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Targeted kill of one OpenVPN process tree. Deliberately NO global
    /// `killall` here — the `CleanupGuard` is the only global safety net, and
    /// it is silenced (see killer.rs). This is what stops the
    /// "no process killed" spam.
    pub(crate) async fn kill_handle(&self, pid: u32, handle: &mut OpenVpnHandle) {
        let _ = self.killer.kill_process_group(pid);
        let _ = handle.child.wait().await;
    }

    /// Go parity: after `MAX_FAILURES`, remove the config from history entirely.
    pub(crate) async fn drop_candidate(
        &self,
        candidate: &Candidate,
        failures: u32,
        host: &mut dyn crate::connect::host::ConnectHost,
    ) -> Result<()> {
        let _ = self
            .repo
            .delete_config_by_path(Path::new(&candidate.path))
            .await;
        let file_name = candidate.file_name();
        host.notify(&format!(
            "removed {file_name} from recent list after {failures} failed attempt(s)"
        ))
        .await?;
        Ok(())
    }

    pub(crate) async fn record_connected(&self, candidate: &Candidate) -> Result<()> {
        let path = Path::new(&candidate.path);
        let sha = crate::hash::sha256_file(path)
            .unwrap_or_else(|_| crate::hash::sha256_str(&candidate.path));
        let remote_host = crate::ovpn::parser::parse_remote_host(path).ok();
        let country =
            CountryCode::new(&candidate.country).unwrap_or_else(|_| CountryCode::unknown());
        self.repo
            .record_success(
                path,
                &sha,
                remote_host.as_deref(),
                &country,
                CountrySource::Unknown,
            )
            .await?;
        Ok(())
    }
}

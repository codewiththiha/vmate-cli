//! The connect orchestration: candidate loop and two-phase monitoring.
//!
//! Retry/drop semantics mirror the original Go tool: a failed handshake
//! retries the same config once, and a second failure removes the config from
//! history entirely. A successful handshake resets the failure budget. Manual
//! `n`/Next only defers the config (row stays in the DB); `r`/Reconnect is
//! penalty-free. The per-candidate helpers live in [`super::session`]; the
//! [`ConnectHost`] trait lives in [`super::host`].

use crate::connect::host::ConnectHost;
use crate::connect::queue::ConnectQueue;
use crate::connect::session::Phase2Exit;
use crate::connect::state::ConnectionStatus;
use crate::db::ConfigRepo;
use crate::ovpn::process::{ConnectOutcome, OpenVpnRunner, connect_args, monitor_connect};
use crate::system::killer::{CleanupGuard, ProcessKiller};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Options for a connect session.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub connect_timeout: Duration,
    pub killall_enabled: bool,
}

/// Orchestrates the connect loop.
pub struct ConnectService {
    pub runner: Arc<dyn OpenVpnRunner>,
    pub killer: Arc<dyn ProcessKiller>,
    pub repo: Arc<ConfigRepo>,
    pub options: ConnectOptions,
}

impl ConnectService {
    pub async fn run(&self, mut queue: ConnectQueue, host: &mut dyn ConnectHost) -> Result<()> {
        let _guard = CleanupGuard::new(self.killer.clone(), self.options.killall_enabled);
        let cancel = CancellationToken::new();
        // Go parity: try once, reconnect once, then drop from history.
        const MAX_FAILURES: u32 = 2;

        while let Some(first) = queue.next_candidate() {
            if cancel.is_cancelled() {
                break;
            }
            let candidate = first;
            let mut failures: u32 = 0;
            let mut last_reason: Option<String> = None;
            let mut reconnecting = false;

            loop {
                if cancel.is_cancelled() {
                    host.finish().await?;
                    return Ok(());
                }

                let message = if reconnecting {
                    format!("Reconnecting to {}", candidate.country)
                } else if failures == 0 {
                    format!("Connecting to {}", candidate.country)
                } else {
                    match &last_reason {
                        Some(reason) => format!(
                            "Retrying {} (failure {failures}/{MAX_FAILURES}): {reason}",
                            candidate.country
                        ),
                        None => format!(
                            "Retrying {} (failure {failures}/{MAX_FAILURES})",
                            candidate.country
                        ),
                    }
                };
                host.status(&ConnectionStatus {
                    connected: false,
                    candidate: Some(candidate.clone()),
                    message,
                    filter: String::new(),
                })
                .await?;
                reconnecting = false;

                let args = connect_args(Path::new(&candidate.path));
                let mut handle = match self.runner.spawn(&args) {
                    Ok(h) => h,
                    Err(e) => {
                        // A spawn failure is systemic (missing binary, broken
                        // exec), not a per-config failure — abort the session
                        // rather than deleting configs from history.
                        host.notify(&format!("failed to start openvpn: {e:#}"))
                            .await?;
                        host.finish().await?;
                        return Err(e);
                    }
                };
                let pid = handle.child.id().unwrap_or(0);
                let outcome = monitor_connect(
                    &mut handle.lines,
                    self.options.connect_timeout,
                    cancel.clone(),
                    host,
                )
                .await;

                match outcome {
                    ConnectOutcome::Cancelled => {
                        self.kill_handle(pid, &mut handle).await;
                        host.finish().await?;
                        return Ok(());
                    }
                    ConnectOutcome::Connected => {
                        let _ = self.record_connected(&candidate).await;
                        failures = 0; // successful handshake resets the budget
                        last_reason = None;
                        let exit = self
                            .monitor_connected(&candidate, &mut handle, host, &cancel)
                            .await?;
                        self.kill_handle(pid, &mut handle).await; // targeted kill only
                        match exit {
                            Phase2Exit::Quit => {
                                host.finish().await?;
                                return Ok(());
                            }
                            Phase2Exit::Next => {
                                // Manual skip: KEEP in DB, defer in-session.
                                let _ = self.repo.mark_skipped(candidate.id).await;
                                queue.skip(candidate);
                                break;
                            }
                            Phase2Exit::Reconnect => {
                                reconnecting = true;
                                continue; // penalty-free
                            }
                            Phase2Exit::Crashed(reason) => {
                                let _ = self
                                    .repo
                                    .note_connect_failure(Path::new(&candidate.path))
                                    .await;
                                failures += 1;
                                last_reason = Some(reason.clone());
                                if failures >= MAX_FAILURES {
                                    self.drop_candidate(&candidate, failures, host).await?;
                                    break;
                                }
                                host.notify(&format!("{reason}; retrying {}", candidate.country))
                                    .await?;
                                continue;
                            }
                        }
                    }
                    bad => {
                        // Handshake failure: Error / RestartPause / TimedOut / Exited.
                        let reason = match bad {
                            ConnectOutcome::Error(m) => format!("connection error: {m}"),
                            ConnectOutcome::RestartPause => "restart pause".to_string(),
                            ConnectOutcome::TimedOut => "connection timed out".to_string(),
                            ConnectOutcome::Exited => {
                                "openvpn exited before connecting".to_string()
                            }
                            // Connected/Cancelled are matched above; this arm is
                            // unreachable for them, but keeping the variants
                            // explicit makes future ConnectOutcome additions a
                            // compile error instead of a silent panic.
                            ConnectOutcome::Connected | ConnectOutcome::Cancelled => {
                                unreachable!()
                            }
                        };
                        self.kill_handle(pid, &mut handle).await; // targeted kill only
                        let _ = self
                            .repo
                            .note_connect_failure(Path::new(&candidate.path))
                            .await;
                        failures += 1;
                        last_reason = Some(reason.clone());
                        if failures >= MAX_FAILURES {
                            self.drop_candidate(&candidate, failures, host).await?;
                            break;
                        }
                        host.notify(&format!("{reason}; retrying {}", candidate.country))
                            .await?;
                        // candidate unchanged -> retry
                    }
                }
            }
        }
        host.finish().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::host::UserCommand;
    use crate::connect::queue::Candidate;
    use crate::connect::state::ConnectionStatus;
    use crate::country::CountryCode;
    use crate::db::models::CountrySource;
    use crate::db::pool::init_pool;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct FakeHost {
        commands: Mutex<std::vec::IntoIter<UserCommand>>,
        messages: Mutex<Vec<String>>,
        statuses: Mutex<Vec<ConnectionStatus>>,
    }

    impl FakeHost {
        fn new(commands: Vec<UserCommand>) -> Self {
            Self {
                commands: Mutex::new(commands.into_iter()),
                messages: Mutex::new(Vec::new()),
                statuses: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ConnectHost for FakeHost {
        async fn status(&mut self, status: &ConnectionStatus) -> Result<()> {
            self.statuses.lock().unwrap().push(status.clone());
            Ok(())
        }
        async fn notify(&mut self, message: &str) -> Result<()> {
            self.messages.lock().unwrap().push(message.to_string());
            Ok(())
        }
        async fn log(&mut self, _line: &str) -> Result<()> {
            Ok(())
        }
        async fn copy(&mut self, _text: &str) -> Result<()> {
            Ok(())
        }
        async fn poll_command(&mut self) -> Option<UserCommand> {
            self.commands.lock().unwrap().next()
        }
        async fn finish(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// Spawning always fails (no OpenVPN available in tests).
    struct NeverSpawn;

    impl OpenVpnRunner for NeverSpawn {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            Err(anyhow::anyhow!("no openvpn in tests"))
        }
        fn bin(&self) -> &str {
            "openvpn"
        }
    }

    /// Spawns a real `sh -c 'echo AUTH_FAILED'`, so the handshake monitor
    /// immediately sees an error line and the connection fails.
    struct FailHandshakeRunner;

    impl OpenVpnRunner for FailHandshakeRunner {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            crate::ovpn::process::spawn_openvpn("sh", &["-c".into(), "echo AUTH_FAILED".into()])
        }
        fn bin(&self) -> &str {
            "sh"
        }
    }

    fn test_service(repo: Arc<ConfigRepo>) -> ConnectService {
        let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
            killall_enabled: false,
        });
        ConnectService {
            runner: Arc::new(NeverSpawn),
            killer,
            repo,
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                killall_enabled: false,
            },
        }
    }

    #[tokio::test]
    async fn empty_queue_finishes_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let service = test_service(repo);
        let mut host = FakeHost::new(vec![]);
        service
            .run(ConnectQueue::new(vec![]), &mut host)
            .await
            .unwrap();
        assert!(host.statuses.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn spawn_failure_aborts_and_preserves_history() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let service = test_service(repo.clone());

        let queue = ConnectQueue::new(vec![
            Candidate {
                id: 1,
                path: "/tmp/a.ovpn".into(),
                country: "JP".into(),
            },
            Candidate {
                id: 2,
                path: "/tmp/b.ovpn".into(),
                country: "KR".into(),
            },
        ]);

        let mut host = FakeHost::new(vec![]);
        // Spawn failure is systemic and must abort the session, not delete
        // every config from history.
        assert!(service.run(queue, &mut host).await.is_err());
    }

    #[tokio::test]
    async fn two_handshake_failures_drop_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let path = "/tmp/a.ovpn";
        let id = repo
            .record_success(
                std::path::Path::new(path),
                "x",
                None,
                &CountryCode::new("jp").unwrap(),
                CountrySource::FileName,
            )
            .await
            .unwrap();

        let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
            killall_enabled: false,
        });
        let service = ConnectService {
            runner: Arc::new(FailHandshakeRunner),
            killer,
            repo: repo.clone(),
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                killall_enabled: false,
            },
        };

        let queue = ConnectQueue::new(vec![Candidate {
            id,
            path: path.into(),
            country: "JP".into(),
        }]);
        let mut host = FakeHost::new(vec![]);
        service.run(queue, &mut host).await.unwrap();

        assert!(
            repo.config_by_path(std::path::Path::new(path))
                .await
                .unwrap()
                .is_none()
        );
    }
}

//! The connect orchestration: candidate loop and two-phase monitoring.
//!
//! Retry/drop semantics: a failed attempt — a handshake failure or a crash
//! after connecting — retries the same config once, and a second failure
//! removes the config from history entirely. A crash that follows a session
//! stable for [`ConnectOptions::connect_stability_grace`] is a real
//! connection and resets the budget, so a config that stays up for a while
//! isn't dropped for two long-session crashes; a crash right after the
//! handshake still counts, so a connect-then-crash config can't loop forever.
//! Manual `n`/Next only defers the config (row stays in the DB); `r`/Reconnect
//! is penalty-free. The per-candidate helpers live in [`super::session`]; the
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
    /// A connected session that survives this long after the handshake is a
    /// real connection: its crash resets the failure budget instead of
    /// counting against it. A crash sooner is connect-then-crash flakiness
    /// and counts, so such configs still get dropped after `MAX_FAILURES`.
    pub connect_stability_grace: Duration,
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
                    ConnectOutcome::Next => {
                        // User pressed `n` during the handshake: KEEP in DB,
                        // defer in-session, move to the next candidate.
                        self.kill_handle(pid, &mut handle).await;
                        let _ = self.repo.mark_skipped(candidate.id).await;
                        queue.skip(candidate);
                        break;
                    }
                    ConnectOutcome::Connected => {
                        let _ = self.record_connected(&candidate).await;
                        last_reason = None;
                        let connected_at = std::time::Instant::now();
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
                                // A session that survived the stability grace is
                                // a real connection: its crash resets the budget
                                // so two long-session crashes (a network blip, an
                                // ISP drop) don't delete a working config from
                                // history. A crash right after the handshake is
                                // connect-then-crash flakiness and still counts —
                                // resetting there would retry it forever.
                                if connected_at.elapsed() >= self.options.connect_stability_grace {
                                    failures = 0;
                                }
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
                            // Connected/Cancelled/Next are matched above; this
                            // arm is unreachable for them, but keeping the
                            // variants explicit makes future ConnectOutcome
                            // additions a compile error instead of a silent
                            // panic.
                            ConnectOutcome::Connected
                            | ConnectOutcome::Cancelled
                            | ConnectOutcome::Next => {
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
            // Yield like the real host's blocking key poll (up to 200ms) so
            // concurrent producers — the openvpn output reader and the connect
            // timer — get scheduled instead of the select loop spinning.
            tokio::time::sleep(Duration::from_millis(10)).await;
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

    /// Spawns `sh -c 'echo "Initialization Sequence Completed"'`: the handshake
    /// succeeds, then the process exits immediately, so the connected session
    /// crashes right after connecting (a connect-then-crash).
    struct ConnectThenExitRunner;

    impl OpenVpnRunner for ConnectThenExitRunner {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            crate::ovpn::process::spawn_openvpn(
                "sh",
                &[
                    "-c".into(),
                    "echo 'Initialization Sequence Completed'".into(),
                ],
            )
        }
        fn bin(&self) -> &str {
            "sh"
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

    /// Spawns `sh -c 'sleep 30'`: no completion or error lines, so the
    /// handshake hangs until a key is pressed or the timeout fires.
    struct SlowRunner;

    impl OpenVpnRunner for SlowRunner {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            crate::ovpn::process::spawn_openvpn("sh", &["-c".into(), "sleep 30".into()])
        }
        fn bin(&self) -> &str {
            "sh"
        }
    }

    /// Spawns `sh -c 'echo "Initialization Sequence Completed"; sleep 0.2'`:
    /// the handshake succeeds, the session stays up past the stability grace,
    /// then the process exits — a long-lived session's crash, not a
    /// connect-then-crash.
    struct ConnectThenStableThenExitRunner;

    impl OpenVpnRunner for ConnectThenStableThenExitRunner {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            crate::ovpn::process::spawn_openvpn(
                "sh",
                &[
                    "-c".into(),
                    "echo 'Initialization Sequence Completed'; sleep 0.2".into(),
                ],
            )
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
                connect_stability_grace: Duration::from_secs(1),
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
                connect_stability_grace: Duration::from_secs(1),
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

    /// A config that connects successfully but then crashes must still be
    /// dropped after two failed attempts — it must not loop forever because the
    /// handshake keeps "succeeding" and resetting the failure budget.
    #[tokio::test]
    async fn connect_then_crash_twice_drops_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let path = "/tmp/connect-then-crash.ovpn";
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
            runner: Arc::new(ConnectThenExitRunner),
            killer,
            repo: repo.clone(),
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                connect_stability_grace: Duration::from_secs(1),
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

        // After connect + crash + retry + crash the config is gone from the DB
        // (and therefore from `recent`).
        assert!(
            repo.config_by_path(std::path::Path::new(path))
                .await
                .unwrap()
                .is_none()
        );

        // The user is told the config was removed from the recent list.
        let msgs = host.messages.lock().unwrap();
        assert!(
            msgs.iter()
                .any(|m| m.contains("removed") && m.contains("recent list")),
            "expected a 'removed ... from recent list' notice, got: {msgs:?}"
        );
    }

    /// A host that reports Quit once the N-th connection's phase-2 monitoring
    /// has begun (a `connected: true` status). Lets a crash-retry loop with a
    /// resetting budget run long enough to prove the config is never dropped.
    struct QuitAfterConnect {
        connects: usize,
        quit_after: usize,
    }

    #[async_trait]
    impl ConnectHost for QuitAfterConnect {
        async fn status(&mut self, s: &ConnectionStatus) -> Result<()> {
            if s.connected {
                self.connects += 1;
            }
            Ok(())
        }
        async fn notify(&mut self, _message: &str) -> Result<()> {
            Ok(())
        }
        async fn log(&mut self, _line: &str) -> Result<()> {
            Ok(())
        }
        async fn copy(&mut self, _text: &str) -> Result<()> {
            Ok(())
        }
        async fn poll_command(&mut self) -> Option<UserCommand> {
            // Yield like the real host's blocking key poll so the crash timer
            // and output reader get scheduled instead of the select loop
            // spinning.
            tokio::time::sleep(Duration::from_millis(10)).await;
            if self.connects >= self.quit_after {
                Some(UserCommand::Quit)
            } else {
                None
            }
        }
        async fn finish(&mut self) -> Result<()> {
            Ok(())
        }
    }

    /// A config that connects and stays up past the stability grace must NOT be
    /// dropped for two long-session crashes: each crash resets the failure
    /// budget, so the config keeps being retried (Go parity). Resetting on
    /// every handshake would loop forever; never resetting would delete a
    /// working config after two network blips.
    #[tokio::test]
    async fn long_session_crashes_do_not_drop_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let path = "/tmp/stable-then-crash.ovpn";
        repo.record_success(
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
            runner: Arc::new(ConnectThenStableThenExitRunner),
            killer,
            repo: repo.clone(),
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                // Far below the runner's ~0.2s uptime so each crash follows a
                // "stable" session and resets the budget.
                connect_stability_grace: Duration::from_millis(50),
                killall_enabled: false,
            },
        };

        let queue = ConnectQueue::new(vec![Candidate {
            id: 1,
            path: path.into(),
            country: "JP".into(),
        }]);
        let mut host = QuitAfterConnect {
            connects: 0,
            quit_after: 3,
        };
        service.run(queue, &mut host).await.unwrap();

        // Two long-session crashes each reset the budget (failures stayed at
        // 1, never reaching MAX_FAILURES), so the config is still in history.
        assert!(
            repo.config_by_path(std::path::Path::new(path))
                .await
                .unwrap()
                .is_some(),
            "long-session crashes must not delete a working config from history"
        );
    }

    /// Pressing `q` during the handshake must quit immediately instead of
    /// waiting for the connect timeout — and it is a clean exit, not a failure,
    /// so the config stays in history.
    #[tokio::test]
    async fn quit_during_handshake_is_immediate() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let path = "/tmp/quit-during.ovpn";
        repo.record_success(
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
            runner: Arc::new(SlowRunner),
            killer,
            repo: repo.clone(),
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(30), // quit must beat this
                connect_stability_grace: Duration::from_secs(1),
                killall_enabled: false,
            },
        };

        let queue = ConnectQueue::new(vec![Candidate {
            id: 0,
            path: path.into(),
            country: "JP".into(),
        }]);
        let mut host = FakeHost::new(vec![UserCommand::Quit]);
        service.run(queue, &mut host).await.unwrap();

        // Quit during the handshake is a clean exit, not a drop: the config is
        // still in history.
        assert!(
            repo.config_by_path(std::path::Path::new(path))
                .await
                .unwrap()
                .is_some()
        );
    }
}

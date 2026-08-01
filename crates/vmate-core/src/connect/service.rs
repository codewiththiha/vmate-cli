//! The connect session: candidate loop, two-phase monitoring and interactive
//! commands. The UI is abstracted behind [`ConnectHost`] so the logic stays
//! testable.

use crate::connect::queue::{Candidate, ConnectQueue};
use crate::connect::state::ConnectionStatus;
use crate::country::CountryCode;
use crate::db::ConfigRepo;
use crate::db::models::CountrySource;
use crate::ovpn::monitor::{VpnLineClass, classify_line};
use crate::ovpn::process::{ConnectOutcome, OpenVpnRunner, connect_args, monitor_connect};
use crate::system::killer::{CleanupGuard, ProcessKiller};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// A user command produced by the interactive UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCommand {
    Next,
    Reconnect,
    CopyPath,
    ToggleVerbose,
    Help,
    Quit,
}

/// The UI surface the connect service drives.
#[async_trait]
pub trait ConnectHost: Send {
    /// Render a state change.
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()>;
    /// Show a transient message.
    async fn notify(&mut self, message: &str) -> Result<()>;
    /// Show a verbose OpenVPN output line.
    async fn log(&mut self, line: &str) -> Result<()>;
    /// Copy text to the clipboard.
    async fn copy(&mut self, text: &str) -> Result<()>;
    /// Poll for a key without blocking indefinitely (re-renders meanwhile).
    async fn poll_command(&mut self) -> Option<UserCommand>;
    /// Restore the terminal when the session ends.
    async fn finish(&mut self) -> Result<()>;
}

/// Options for a connect session.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub connect_timeout: Duration,
    pub verbose: bool,
    pub killall_enabled: bool,
    pub max_retries: Option<u32>,
}

/// Orchestrates the connect loop.
pub struct ConnectService {
    pub runner: Arc<dyn OpenVpnRunner>,
    pub killer: Arc<dyn ProcessKiller>,
    pub repo: Arc<ConfigRepo>,
    pub options: ConnectOptions,
}

enum MonitorExit {
    Quit,
    Next,
    Failed(String),
}

impl ConnectService {
    pub async fn run(&self, mut queue: ConnectQueue, host: &mut dyn ConnectHost) -> Result<()> {
        let _guard = CleanupGuard::new(self.killer.clone(), self.options.killall_enabled);
        let cancel = CancellationToken::new();
        let filter_label = String::new();
        let mut attempts: u32 = 0;
        let mut current: Option<Candidate> = None;

        loop {
            let candidate = match current.take() {
                Some(c) => c,
                None => match queue.next_candidate() {
                    Some(c) => c,
                    None => break,
                },
            };

            if let Some(max) = self.options.max_retries {
                if attempts >= max {
                    host.notify(&format!("reached maximum retries ({max})"))
                        .await?;
                    break;
                }
            }
            attempts += 1;

            host.status(&ConnectionStatus {
                connected: false,
                candidate: Some(candidate.clone()),
                message: format!("Connecting to {}", candidate.country),
                filter: filter_label.clone(),
            })
            .await?;

            let args = connect_args(Path::new(&candidate.path));
            let mut handle = match self.runner.spawn(&args) {
                Ok(h) => h,
                Err(e) => {
                    host.notify(&format!("failed to start openvpn: {e:#}"))
                        .await?;
                    continue;
                }
            };
            let pid = handle.child.id().unwrap_or(0);

            let outcome = monitor_connect(
                &mut handle.lines,
                self.options.connect_timeout,
                cancel.clone(),
            )
            .await;

            match outcome {
                ConnectOutcome::Connected => {
                    let _ = self.record_connected(&candidate).await;

                    host.status(&ConnectionStatus {
                        connected: true,
                        candidate: Some(candidate.clone()),
                        message: format!("Connected successfully to {}", candidate.country),
                        filter: filter_label.clone(),
                    })
                    .await?;

                    let mut verbose = self.options.verbose;
                    let mut exit = MonitorExit::Quit;
                    let mut reconnect = false;

                    // Phase 2: indefinite monitoring with interactive keys.
                    loop {
                        tokio::select! {
                            _ = cancel.cancelled() => {
                                exit = MonitorExit::Quit;
                                break;
                            }
                            cmd = host.poll_command() => {
                                match cmd {
                                    Some(UserCommand::Next) => {
                                        exit = MonitorExit::Next;
                                        break;
                                    }
                                    Some(UserCommand::Reconnect) => {
                                        reconnect = true;
                                        break;
                                    }
                                    Some(UserCommand::CopyPath) => {
                                        let _ = host.copy(&candidate.path).await;
                                    }
                                    Some(UserCommand::ToggleVerbose) => {
                                        verbose = !verbose;
                                    }
                                    Some(UserCommand::Help) => {
                                        let _ = host
                                            .notify("[n] next  [r] reconnect  [c] copy path  [v] verbose  [q] quit")
                                            .await;
                                    }
                                    Some(UserCommand::Quit) => {
                                        exit = MonitorExit::Quit;
                                        break;
                                    }
                                    None => {}
                                }
                            }
                            line = handle.lines.recv() => {
                                let Some(line) = line else {
                                    exit = MonitorExit::Failed("openvpn exited unexpectedly".to_string());
                                    break;
                                };
                                if verbose {
                                    let _ = host.log(&line).await;
                                }
                                match classify_line(&line) {
                                    VpnLineClass::Error(kw) => {
                                        exit = MonitorExit::Failed(format!(
                                            "connection error: {kw}"
                                        ));
                                        break;
                                    }
                                    VpnLineClass::RestartPause => {
                                        exit = MonitorExit::Failed("restart pause".to_string());
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Clean up the current connection before moving on.
                    let _ = self.killer.kill_process_group(pid);
                    let _ = handle.child.wait().await;
                    let _ = self.killer.killall_openvpn();

                    match exit {
                        MonitorExit::Quit => break,
                        MonitorExit::Next => {
                            let _ = self.repo.mark_skipped(candidate.id).await;
                            queue.skip(candidate.clone());
                        }
                        MonitorExit::Failed(msg) => {
                            let _ = self
                                .repo
                                .record_failure(Path::new(&candidate.path), &msg)
                                .await;
                        }
                    }

                    if reconnect {
                        current = Some(candidate);
                    }
                }
                ConnectOutcome::Error(msg) => {
                    self.fail_cleanup(&candidate, pid, &mut handle, &msg, host)
                        .await?;
                }
                ConnectOutcome::RestartPause => {
                    self.fail_cleanup(&candidate, pid, &mut handle, "restart pause", host)
                        .await?;
                }
                ConnectOutcome::TimedOut => {
                    self.fail_cleanup(&candidate, pid, &mut handle, "connection timed out", host)
                        .await?;
                }
                ConnectOutcome::Exited => {
                    self.fail_cleanup(
                        &candidate,
                        pid,
                        &mut handle,
                        "openvpn exited before connecting",
                        host,
                    )
                    .await?;
                }
                ConnectOutcome::Cancelled => {
                    let _ = self.killer.kill_process_group(pid);
                    let _ = handle.child.wait().await;
                    break;
                }
            }
        }

        host.finish().await?;
        Ok(())
    }

    async fn fail_cleanup(
        &self,
        candidate: &Candidate,
        pid: u32,
        handle: &mut crate::ovpn::process::OpenVpnHandle,
        reason: &str,
        host: &mut dyn ConnectHost,
    ) -> Result<()> {
        let _ = self
            .repo
            .record_failure(Path::new(&candidate.path), reason)
            .await;
        let _ = self.killer.kill_process_group(pid);
        let _ = handle.child.wait().await;
        let _ = self.killer.killall_openvpn();
        host.notify(&format!("{reason}: {}", candidate.country))
            .await?;
        Ok(())
    }

    async fn record_connected(&self, candidate: &Candidate) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect::queue::Candidate;
    use crate::db::pool::init_pool;
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

    struct NeverSpawn;

    impl OpenVpnRunner for NeverSpawn {
        fn spawn(&self, _args: &[String]) -> Result<crate::ovpn::process::OpenVpnHandle> {
            Err(anyhow::anyhow!("no openvpn in tests"))
        }
        fn bin(&self) -> &str {
            "openvpn"
        }
    }

    #[tokio::test]
    async fn empty_queue_finishes_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
            killall_enabled: false,
        });
        let service = ConnectService {
            runner: Arc::new(NeverSpawn),
            killer,
            repo,
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                verbose: false,
                killall_enabled: false,
                max_retries: None,
            },
        };

        let mut host = FakeHost::new(vec![]);
        service
            .run(ConnectQueue::new(vec![]), &mut host)
            .await
            .unwrap();
        assert!(host.statuses.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn spawn_failure_moves_to_next_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(ConfigRepo::new(
            init_pool(&dir.path().join("t.db")).await.unwrap(),
        ));
        let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
            killall_enabled: false,
        });
        let service = ConnectService {
            runner: Arc::new(NeverSpawn),
            killer,
            repo,
            options: ConnectOptions {
                connect_timeout: Duration::from_secs(1),
                verbose: false,
                killall_enabled: false,
                max_retries: None,
            },
        };

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
        service.run(queue, &mut host).await.unwrap();
        // Both spawns fail; session ends, finish() called.
        assert_eq!(host.messages.lock().unwrap().len(), 2);
    }
}

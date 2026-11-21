//! The connect orchestration: candidate loop and two-phase monitoring.
//!
//! Retry/drop semantics: a failed attempt — a handshake failure or a crash
//! after connecting — retries the same config up to
//! [`ConnectOptions::retry_count`] failures, and the config is removed from
//! history entirely once that budget is exhausted. A crash that follows a session
//! stable for [`ConnectOptions::connect_stability_grace`] is a real
//! connection and resets the budget, so a config that stays up for a while
//! isn't dropped for two long-session crashes; a crash right after the
//! handshake still counts, so a connect-then-crash config can't loop forever.
//! Manual `n`/Next only defers the config (row stays in the DB); `r`/Reconnect
//! is penalty-free. The per-candidate helpers live in the session helper module;
//! the [`ConnectHost`] trait is re-exported from [`crate::connect`].

use crate::connect::host::ConnectHost;
use crate::connect::queue::ConnectQueue;
use crate::connect::session::Phase2Exit;
use crate::connect::state::ConnectionStatus;
use crate::db::ConfigRepo;
use crate::ovpn::process::{ConnectOutcome, OpenVpnRunner, connect_args, monitor_connect};
use crate::system::killer::{CleanupGuard, ProcessKiller, ProcessRegistry};
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
    /// and counts, so such configs still get dropped after `retry_count`.
    pub connect_stability_grace: Duration,
    /// How many failed attempts a config is allowed before it is dropped from
    /// history (default 2).
    pub retry_count: u32,
    pub killall_enabled: bool,
}

/// What a failed attempt should do with the same config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry the config once more.
    Retry,
    /// Drop the config from history (budget exhausted).
    Drop,
}

/// Pure retry/drop budget: how many failed attempts a config is allowed before
/// it is dropped from history. Synchronous and process-free, so the retry
/// policy is unit-testable without spawning OpenVPN.
#[derive(Debug, Clone, Copy)]
pub struct RetryBudget {
    pub max: u32,
    pub failures: u32,
}

impl RetryBudget {
    /// A budget that drops a config after `max` failed attempts.
    pub fn new(max: u32) -> Self {
        Self {
            max: max.max(1),
            failures: 0,
        }
    }

    /// Reset the counter after a session that proved stable.
    pub fn reset(&mut self) {
        self.failures = 0;
    }

    /// Register one failed attempt and decide whether to retry or drop.
    pub fn on_failure(&mut self) -> RetryDecision {
        self.failures += 1;
        if self.failures >= self.max {
            RetryDecision::Drop
        } else {
            RetryDecision::Retry
        }
    }
}

/// Orchestrates the connect loop.
pub struct ConnectService {
    pub runner: Arc<dyn OpenVpnRunner>,
    pub killer: Arc<dyn ProcessKiller>,
    pub registry: Arc<ProcessRegistry>,
    pub repo: Arc<ConfigRepo>,
    pub options: ConnectOptions,
}

impl ConnectService {
    pub async fn run(&self, mut queue: ConnectQueue, host: &mut dyn ConnectHost) -> Result<()> {
        let cancel = CancellationToken::new();

        let signal_task = crate::system::signal::cancel_on_shutdown(cancel.clone());

        let result = self.run_queue(&mut queue, host, &cancel).await;
        drop(signal_task);
        result
    }

    async fn run_queue(
        &self,
        queue: &mut ConnectQueue,
        host: &mut dyn ConnectHost,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let _guard = CleanupGuard::new(
            self.killer.clone(),
            self.registry.clone(),
            self.options.killall_enabled,
        );
        // Go parity: try once, reconnect once, then drop from history. The
        // budget is configurable via ConnectOptions::retry_count.
        while let Some(first) = queue.next_candidate() {
            if cancel.is_cancelled() {
                break;
            }
            let candidate = first;
            let mut budget = RetryBudget::new(self.options.retry_count);
            let mut last_reason: Option<String> = None;
            let mut reconnecting = false;

            loop {
                if cancel.is_cancelled() {
                    host.finish().await?;
                    return Ok(());
                }

                let message = if reconnecting {
                    format!("Reconnecting to {}", candidate.country)
                } else if budget.failures == 0 {
                    format!("Connecting to {}", candidate.country)
                } else {
                    match &last_reason {
                        Some(reason) => format!(
                            "Retrying {} (failure {}/{max}): {reason}",
                            candidate.country,
                            budget.failures,
                            max = budget.max
                        ),
                        None => format!(
                            "Retrying {} (failure {}/{max})",
                            candidate.country,
                            budget.failures,
                            max = budget.max
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
                let outcome = monitor_connect(
                    &mut handle.lines,
                    self.options.connect_timeout,
                    cancel.clone(),
                    host,
                )
                .await;

                match outcome {
                    ConnectOutcome::Cancelled => {
                        // The user is leaving now: do not make them wait out
                        // the teardown grace period.
                        handle.kill_now(self.killer.as_ref()).await;
                        host.finish().await?;
                        return Ok(());
                    }
                    ConnectOutcome::Next => {
                        // User pressed `n` during the handshake: KEEP in DB,
                        // defer in-session, move to the next candidate.
                        handle.kill_now(self.killer.as_ref()).await;
                        let _ = self.repo.mark_skipped(candidate.id).await;
                        queue.skip(candidate);
                        break;
                    }
                    ConnectOutcome::Connected => {
                        let _ = self.record_connected(&candidate).await;
                        last_reason = None;
                        let connected_at = std::time::Instant::now();
                        let exit = self
                            .monitor_connected(&candidate, &mut handle, host, cancel)
                            .await?;
                        let _ = host.notify("Disconnecting OpenVPN...").await;
                        handle.kill_graceful(self.killer.as_ref()).await;
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
                                    budget.reset();
                                }
                                let _ = self
                                    .repo
                                    .note_connect_failure(Path::new(&candidate.path))
                                    .await;
                                last_reason = Some(reason.clone());
                                if budget.on_failure() == RetryDecision::Drop {
                                    self.drop_candidate(&candidate, budget.failures, host)
                                        .await?;
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
                        handle.kill_graceful(self.killer.as_ref()).await; // targeted kill only
                        let _ = self
                            .repo
                            .note_connect_failure(Path::new(&candidate.path))
                            .await;
                        last_reason = Some(reason.clone());
                        if budget.on_failure() == RetryDecision::Drop {
                            self.drop_candidate(&candidate, budget.failures, host)
                                .await?;
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
#[path = "service/tests/mod.rs"]
mod tests;

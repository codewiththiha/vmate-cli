//! Coordinate recent-first activation, bounded rescans and reconnects.

mod system;
#[cfg(test)]
mod tests;

use crate::{Port, model::Relay};
use anyhow::{Result, bail};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[async_trait]
trait Backend: Send {
    async fn scan(&mut self) -> Result<Vec<Relay>>;
    async fn attempt(&mut self, relay: &Relay) -> Result<bool>;
    async fn discard(&mut self, relay: &Relay) -> Result<()>;
    async fn pause(&mut self, cycle: u32);
}

pub(super) async fn run(port: &Port) -> Result<()> {
    let snapshot = port.snapshot();
    crate::validate_preferences(&snapshot.preferences)?;
    if !cfg!(target_os = "linux") {
        bail!("Automatic connection requires NetworkManager on Linux. Use setup for macOS.");
    }
    let cancel = port.state.lock().unwrap().operation.clone();
    let recent = crate::platform::recent(port).await?;
    drive(
        &mut port.clone(),
        recent,
        snapshot.preferences.auto_reconnect,
        snapshot.preferences.retry_limit,
        cancel,
    )
    .await
}

async fn drive(
    backend: &mut dyn Backend,
    mut recent: Vec<Relay>,
    reconnect: bool,
    limit: u32,
    cancel: CancellationToken,
) -> Result<()> {
    let mut scans = 0;
    loop {
        for relay in recent.drain(..) {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let established = backend.attempt(&relay).await?;
            if cancel.is_cancelled() {
                return Ok(());
            }
            backend.discard(&relay).await?;
            if established && !reconnect {
                bail!("Connection lost. Connect to try again.");
            }
        }
        if cancel.is_cancelled() {
            return Ok(());
        }
        if scans == limit {
            bail!("Retry limit reached. Connect to try again.");
        }
        if scans > 0 {
            backend.pause(scans).await;
        }
        if cancel.is_cancelled() {
            return Ok(());
        }
        scans += 1;
        recent = backend.scan().await?;
    }
}

//! One owned history watcher, independent of a long-running VPN session.

use crate::{Port, platform};
use std::time::Duration;
use vmate_core::db::ConfigRepo;

pub(super) async fn watch(port: Port, history: ConfigRepo) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = port.shutdown.cancelled() => break,
            _ = tick.tick() => {}
        }
        if port.snapshot().status == "Scanning" {
            continue;
        }
        match platform::recent_from(&history).await {
            Ok(relays) => {
                let mut state = port.state.lock().unwrap();
                if state.snapshot.status != "Scanning" {
                    state.snapshot.relays = relays;
                }
            }
            Err(error) => super::fail(&port, error),
        }
    }
    history.pool().close().await;
}

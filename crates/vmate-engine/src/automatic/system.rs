//! NetworkManager adapter with explicit temporary-profile cleanup.

use super::*;
use crate::{host::Host, platform::output, scan};
use anyhow::Context;
use std::time::Duration;
use vmate_core::db::ConfigRepo;

#[async_trait]
impl Backend for Port {
    async fn scan(&mut self) -> Result<Vec<Relay>> {
        let preferences = self.snapshot().preferences;
        let preset = preferences
            .presets
            .iter()
            .find(|p| p.name == preferences.connect_preset)
            .context("Connect preset is missing")?;
        scan::with_preset(self, preset).await?;
        crate::platform::recent(self).await
    }

    async fn attempt(&mut self, relay: &Relay) -> Result<bool> {
        {
            let mut state = self.state.lock().unwrap();
            state.snapshot.connected = false;
            state.snapshot.status = "Connecting".into();
            state.snapshot.message = relay.name.clone();
        }
        let path = relay.path.to_str().context("Relay path is not UTF-8")?;
        let imported = output(
            "nmcli",
            &["connection", "import", "type", "openvpn", "file", path],
        )
        .await?;
        let uuid = imported
            .split(|c: char| !c.is_ascii_hexdigit() && c != '-')
            .find(|s| s.len() == 36 && s.bytes().filter(|b| *b == b'-').count() == 4)
            .context(
                "NetworkManager did not return an imported profile UUID; inspect system VPN",
            )?;
        let cancel = self.state.lock().unwrap().operation.clone();
        let result = vmate_core::managed::run(uuid, &mut Host(self.clone()), cancel).await;
        let established = self.snapshot().connected;
        self.state.lock().unwrap().snapshot.status = "Check system VPN".into();
        let active = output("nmcli", &["-g", "UUID", "connection", "show", "--active"]).await?;
        if active.lines().any(|line| line.trim() == uuid) {
            bail!("VPN teardown is unconfirmed. Inspect system VPN before retrying.");
        }
        output("nmcli", &["connection", "delete", "uuid", uuid]).await?;
        let mut state = self.state.lock().unwrap();
        state.snapshot.connected = false;
        state.snapshot.status = "Connecting".into();
        if let Err(error) = result {
            let diagnostic = format!("{error:#}").to_lowercase();
            if [
                "not authorized",
                "permission denied",
                "no valid vpn secrets",
                "service was not installed",
            ]
            .iter()
            .any(|message| diagnostic.contains(message))
            {
                return Err(error);
            }
            state.event(format!("{}: {error:#}", relay.name));
        }
        Ok(established)
    }

    async fn discard(&mut self, relay: &Relay) -> Result<()> {
        let pool = vmate_core::db::init_pool(&self.configuration.database).await?;
        ConfigRepo::new(pool)
            .record_failure(&relay.path, "Automatic connection failed or ended")
            .await?;
        self.state
            .lock()
            .unwrap()
            .snapshot
            .relays
            .retain(|r| r.path != relay.path);
        Ok(())
    }

    async fn pause(&mut self, cycle: u32) {
        let cancel = {
            let mut state = self.state.lock().unwrap();
            state.snapshot.status = "Retrying".into();
            state.snapshot.message = format!("Retry loop {}", cycle + 1);
            state.operation.clone()
        };
        tokio::select! {
            _ = cancel.cancelled() => {},
            _ = tokio::time::sleep(Duration::from_secs((u64::from(cycle) * 2).min(30))) => {},
        }
    }
}

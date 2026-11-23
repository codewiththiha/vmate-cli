//! Bounded system discovery, preferences and explicit export/import operations.

use crate::{
    Port,
    model::{Preferences, Profile, Relay},
};
use anyhow::{Context, Result, bail};
use std::{path::PathBuf, time::Duration};
use vmate_core::{db::ConfigRepo, filter::CountryFilter};

pub(super) async fn refresh(port: &Port) -> Result<()> {
    let preferences_path = port.configuration.config_dir.join("desktop.json");
    let preferences: Preferences = if preferences_path.exists() {
        serde_json::from_slice(&std::fs::read(preferences_path)?)?
    } else {
        Preferences::default()
    };
    crate::validate_preferences(&preferences)?;
    let binary = &port.configuration.openvpn;
    let version = output(binary.to_string_lossy().as_ref(), &["--version"]).await;
    let profiles = profiles().await;
    let relays = recent(port).await?;
    let mut state = port.state.lock().unwrap();
    state.snapshot.preferences = preferences;
    state.snapshot.openvpn = match version {
        Ok(v) => v.lines().next().unwrap_or("OpenVPN available").into(),
        Err(e) => format!("OpenVPN unavailable: {e:#}"),
    };
    state.snapshot.system_service = match &profiles {
        Ok(_) => "NetworkManager · existing VPN profiles".into(),
        Err(_) if cfg!(target_os = "macos") => {
            "macOS · import into your existing Tunnelblick installation".into()
        }
        Err(e) => format!("NetworkManager unavailable: {e:#}"),
    };
    state.snapshot.profiles = profiles.unwrap_or_default();
    state.snapshot.relays = relays;
    state.event("Dependencies and saved handshake history refreshed.".into());
    Ok(())
}

async fn profiles() -> Result<Vec<Profile>> {
    if !cfg!(target_os = "linux") {
        bail!("NetworkManager is Linux-only");
    }
    let text = output(
        "nmcli",
        &[
            "-t",
            "--escape",
            "no",
            "-f",
            "UUID,TYPE,NAME",
            "connection",
            "show",
        ],
    )
    .await?;
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, ':');
            let uuid = fields.next()?;
            let kind = fields.next()?;
            let name = fields.next()?;
            (kind == "vpn").then(|| Profile {
                uuid: uuid.into(),
                name: name.into(),
            })
        })
        .collect())
}

pub(super) async fn output(binary: &str, arguments: &[&str]) -> Result<String> {
    let mut command = tokio::process::Command::new(binary);
    command
        .args(arguments)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let result = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .context("system command exceeded 10 seconds")??;
    if !result.status.success() {
        bail!("{}", String::from_utf8_lossy(&result.stderr).trim());
    }
    Ok(String::from_utf8(result.stdout)?)
}

pub(super) fn export(port: &Port, path: PathBuf, destination: PathBuf) -> Result<()> {
    let snapshot = port.snapshot();
    let relay = snapshot
        .relays
        .iter()
        .find(|relay| relay.path == path)
        .context("Select a saved relay first")?;
    if destination.exists() {
        bail!("Choose a new filename; existing files are never overwritten.");
    }
    let mut source = std::fs::File::open(&relay.path)?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)?;
    std::io::copy(&mut source, &mut target)?;
    let mut state = port.state.lock().unwrap();
    state.snapshot.message = format!("Exported {}", destination.display());
    state.event(format!("Config exported to {}", destination.display()));
    Ok(())
}

pub(super) async fn import(port: &Port, path: PathBuf) -> Result<()> {
    let snapshot = port.snapshot();
    let relay = snapshot
        .relays
        .iter()
        .find(|relay| relay.path == path)
        .context("Select a saved relay first")?;
    let path = relay.path.to_str().context("Config path is not UTF-8")?;
    if cfg!(target_os = "linux") {
        output(
            "nmcli",
            &["connection", "import", "type", "openvpn", "file", path],
        )
        .await?;
        refresh(port).await?;
        port.state.lock().unwrap().snapshot.message =
            "Imported. Choose the profile in Connections; authorization follows system policy."
                .into();
    } else {
        output("open", &["-a", "Tunnelblick", path]).await?;
        port.state.lock().unwrap().snapshot.message = "Opened in Tunnelblick. Complete authorization and connection there; vmate does not own that tunnel.".into();
    }
    Ok(())
}

pub(super) fn save(port: &Port, preferences: Preferences) -> Result<()> {
    crate::validate_preferences(&preferences)?;
    let path = port.configuration.config_dir.join("desktop.json");
    std::fs::create_dir_all(path.parent().context("settings parent")?)?;
    std::fs::write(path, serde_json::to_vec_pretty(&preferences)?)?;
    let mut state = port.state.lock().unwrap();
    state.snapshot.preferences = preferences;
    state.snapshot.message = "Preferences saved. Applied to your next scan.".into();
    Ok(())
}

pub(super) async fn recent(port: &Port) -> Result<Vec<Relay>> {
    let repo = ConfigRepo::new(vmate_core::db::init_pool(&port.configuration.database).await?);
    recent_from(&repo).await
}

pub(super) async fn recent_from(repo: &ConfigRepo) -> Result<Vec<Relay>> {
    let relays = repo
        .list_recent(&CountryFilter::new(), Some(500), 0)
        .await?;
    Ok(relays
        .into_iter()
        .map(|row| Relay {
            path: row.path.into(),
            name: row.remote_host.unwrap_or_else(|| "Saved config".into()),
            country: row.country,
            checked: row
                .last_success_at
                .map(|v| v.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_default(),
        })
        .collect())
}

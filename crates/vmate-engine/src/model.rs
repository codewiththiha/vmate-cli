//! Frontend-neutral application state; no toolkit or window handles.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A stored handshake result, never proof of a connected system VPN.
#[derive(Clone, Debug, Default)]
pub struct Relay {
    pub path: PathBuf,
    pub country: String,
    pub name: String,
    pub checked: String,
}

/// Existing system VPN identity; the UUID is the activation key.
#[derive(Clone, Debug)]
pub struct Profile {
    pub uuid: String,
    pub name: String,
}

/// Persistent desktop preferences, separate from CLI defaults.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub presets: Vec<ScanPreset>,
    pub scan_preset: String,
    pub connect_preset: String,
    pub auto_reconnect: bool,
    pub retry_limit: u32,
    pub reduced_motion: bool,
    pub country_lookup: bool,
    /// Opt-in desktop system tray; older preferences default to disabled.
    pub tray_enabled: bool,
    /// System, light or dark presentation; no effect on networking.
    pub appearance: Appearance,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            presets: vec![ScanPreset::default()],
            scan_preset: "Default".into(),
            connect_preset: "Default".into(),
            auto_reconnect: false,
            retry_limit: 3,
            reduced_motion: false,
            country_lookup: true,
            tray_enabled: false,
            appearance: Appearance::System,
        }
    }
}

/// One bounded, internally consistent view of the engine.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub status: String,
    pub message: String,
    pub error: String,
    pub busy: bool,
    pub connected: bool,
    pub connected_country: String,
    pub completed: usize,
    pub total: usize,
    pub relays: Vec<Relay>,
    pub profiles: Vec<Profile>,
    pub events: Vec<String>,
    pub openvpn: String,
    pub system_service: String,
    pub preferences: Preferences,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: "Not connected".into(),
            message: "Scan for relays, then connect through your system VPN service.".into(),
            error: String::new(),
            busy: false,
            connected: false,
            connected_country: String::new(),
            completed: 0,
            total: 0,
            relays: Vec::new(),
            profiles: Vec::new(),
            events: Vec::new(),
            openvpn: "Checking OpenVPN…".into(),
            system_service: "Checking system VPN service…".into(),
            preferences: Preferences::default(),
        }
    }
}

/// Explicit, validated user intent accepted by the application engine.
#[derive(Clone, Debug)]
pub enum Command {
    AutoConnect,
    Refresh,
    Scan,
    LookupCountry { path: PathBuf },
    Connect(String),
    Export { path: PathBuf, destination: PathBuf },
    Import { path: PathBuf },
    Save(Preferences),
}

/// A named, persisted scan configuration assigned independently to actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanPreset {
    pub name: String,
    pub workers: usize,
    pub timeout: u64,
}

impl Default for ScanPreset {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            workers: 64,
            timeout: 5,
        }
    }
}

/// Persisted color preference independent of the GUI toolkit.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

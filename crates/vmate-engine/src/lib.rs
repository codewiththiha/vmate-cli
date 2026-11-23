//! Toolkit-independent application engine with one owned asynchronous worker.

mod automatic;
mod country;
pub mod export;
mod host;
pub mod model;
mod operations;
mod platform;
mod scan;

/// Explicit storage and executable paths make engine instances testable.
#[derive(Clone, Debug)]
pub struct Configuration {
    pub config_dir: std::path::PathBuf,
    pub database: std::path::PathBuf,
    pub openvpn: std::path::PathBuf,
}

impl Configuration {
    /// Resolve installed application paths without issuing network requests.
    pub fn system() -> Result<Self> {
        if vmate_core::system::is_root() {
            bail!(
                "Run the desktop as your normal user; authorize VPN changes through the system service."
            );
        }
        let database = std::env::var_os("VMATE_DB").map(std::path::PathBuf::from);
        Ok(Self {
            config_dir: vmate_core::paths::config_dir()?,
            database: vmate_core::paths::database_path(database.as_deref())?,
            openvpn: vmate_core::ovpn::binary::resolve(None)?,
        })
    }
}

use anyhow::{Result, bail};
use model::{Command, Snapshot};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Shared bounded state owned by an engine instance.
pub(crate) struct State {
    pub snapshot: Snapshot,
    pub operation: CancellationToken,
}

impl State {
    fn diagnose(&mut self, error: anyhow::Error) {
        let text = format!("{error:#}");
        self.snapshot.error = text.chars().take(8192).collect();
        self.event(text);
    }

    pub fn event(&mut self, text: String) {
        if self.snapshot.events.len() == 128 {
            self.snapshot.events.remove(0);
        }
        self.snapshot.events.push(text.chars().take(2048).collect());
    }
}

/// Cloneable command port; cancellation does not depend on queue capacity.
#[derive(Clone)]
pub struct Port {
    sender: mpsc::Sender<Command>,
    state: Arc<Mutex<State>>,
    shutdown: CancellationToken,
    configuration: Arc<Configuration>,
}

impl Port {
    /// Submit without blocking a frontend event loop.
    pub fn send(&self, command: Command) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if self.shutdown.is_cancelled() {
            bail!("Engine is shutting down.");
        }
        let saving = matches!(&command, Command::Save(_));
        let exporting = matches!(
            &command,
            Command::Export { .. } | Command::ExportBatch { .. }
        );
        if saving || exporting {
            if (saving && state.snapshot.saving) || (exporting && state.snapshot.exporting) {
                bail!("This action is already in progress.");
            }
            self.sender
                .try_send(command)
                .map_err(|e| anyhow::anyhow!("engine queue: {e}"))?;
            state.snapshot.saving |= saving;
            state.snapshot.exporting |= exporting;
            return Ok(());
        }
        if state.snapshot.busy {
            bail!("Finish or cancel the current operation first.");
        }
        state.operation = self.shutdown.child_token();
        self.sender
            .try_send(command)
            .map_err(|e| anyhow::anyhow!("engine queue: {e}"))?;
        state.snapshot.busy = true;
        Ok(())
    }

    /// Read the latest bounded state without consuming an event stream.
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().snapshot.clone()
    }

    /// Cancel and await teardown via subsequent busy=false snapshots.
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        state.operation.cancel();
        if state.snapshot.busy {
            state.snapshot.message = "Disconnecting and reaping owned processes…".into();
        }
    }

    /// Let a frontend leave its event loop after shutdown teardown completes.
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.is_cancelled()
    }

    /// Stop accepting work and cancel the active operation.
    pub fn shutdown(&self) {
        self.shutdown.cancel();
        self.cancel();
    }
}

/// Owns the worker thread; dropping joins teardown before releasing the runtime.
pub struct Engine {
    port: Port,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Engine {
    /// Start the engine without issuing scans or requesting authorization.
    pub fn start(configuration: Configuration) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(16);
        let shutdown = CancellationToken::new();
        let state = Arc::new(Mutex::new(State {
            snapshot: Snapshot::default(),
            operation: shutdown.child_token(),
        }));
        let port = Port {
            sender,
            state,
            shutdown,
            configuration: Arc::new(configuration),
        };
        let actor = port.clone();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let worker = std::thread::Builder::new()
            .name("vmate-engine".into())
            .spawn(move || {
                runtime.block_on(operations::run(actor, receiver));
            })?;
        Ok(Self {
            port,
            worker: Some(worker),
        })
    }

    /// The same port can be bound to Slint or a different frontend.
    pub fn port(&self) -> Port {
        self.port.clone()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.port.shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Check resource limits before starting any worker.
pub fn validate_preferences(p: &model::Preferences) -> Result<()> {
    if !(1..=20).contains(&p.retry_limit) || p.presets.is_empty() || p.presets.len() > 16 {
        bail!("Use 1–20 retry loops and 1–16 presets.");
    }
    let mut names = std::collections::HashSet::new();
    for preset in &p.presets {
        if preset.name.trim().is_empty()
            || preset.name.len() > 48
            || !names.insert(&preset.name)
            || !(1..=500).contains(&preset.workers)
            || !(1..=120).contains(&preset.timeout)
        {
            bail!("Preset names must be unique; use 1–500 workers and 1–120 seconds.");
        }
    }
    if !names.contains(&p.scan_preset) || !names.contains(&p.connect_preset) {
        bail!("Assign an existing preset to Scan and Connect.");
    }
    Ok(())
}

#[cfg(test)]
mod tests;

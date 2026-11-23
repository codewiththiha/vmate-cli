//! Interleave settings and export work with one owned VPN operation.

mod history;

use crate::{Port, host::Host, model::Command, platform, scan};
use anyhow::Result;
use tokio::sync::mpsc;

pub(super) async fn run(port: Port, mut receiver: mpsc::Receiver<Command>) {
    let _signal = vmate_core::system::signal::cancel_on_shutdown(port.shutdown.clone());
    if let Err(error) = platform::refresh(&port).await {
        fail(&port, error);
    }
    port.state.lock().unwrap().snapshot.ready = true;
    let pool = match vmate_core::db::init_pool(&port.configuration.database).await {
        Ok(pool) => pool,
        Err(error) => {
            fail(&port, error);
            return;
        }
    };
    let history = vmate_core::db::ConfigRepo::new(pool);
    let watcher = tokio::spawn(history::watch(port.clone(), history));
    loop {
        let command = tokio::select! {
            biased;
            _ = port.shutdown.cancelled() => break,
            command = receiver.recv() => match command { Some(c) => c, None => break },
        };
        let Some(command) = maintenance(&port, command) else {
            continue;
        };
        {
            let mut state = port.state.lock().unwrap();
            state.snapshot.busy = true;
            state.snapshot.error.clear();
            state.snapshot.connected_country.clear();
        }
        let managed = matches!(&command, Command::Connect(_));
        let automatic = matches!(&command, Command::AutoConnect);
        let result = active(&port, &mut receiver, command).await;
        let uncertain = result.is_err()
            && (managed || (automatic && port.snapshot().status == "Check system VPN"));
        if let Err(error) = result {
            fail(&port, error);
        }
        let mut state = port.state.lock().unwrap();
        state.snapshot.busy = false;
        state.snapshot.connected = false;
        state.snapshot.connected_country.clear();
        state.snapshot.status = if uncertain {
            "Check system VPN"
        } else {
            "Not connected"
        }
        .into();
        if state.operation.is_cancelled() && !uncertain {
            state.snapshot.message = "Operation stopped.".into();
        }
        if uncertain {
            state.snapshot.message =
                "Inspect NetworkManager to confirm tunnel state before retrying.".into();
        }
    }
    if let Err(error) = watcher.await {
        fail(&port, error.into());
    }
}

async fn execute(port: &Port, command: Command) -> Result<()> {
    match command {
        Command::AutoConnect => crate::automatic::run(port).await,
        Command::Refresh => platform::refresh(port).await,
        Command::Scan => scan::run(port).await,
        Command::LookupCountry { path } => crate::country::lookup(port, &path).await,
        Command::Connect(uuid) => {
            let cancel = port.state.lock().unwrap().operation.clone();
            vmate_core::managed::run(&uuid, &mut Host(port.clone()), cancel).await
        }
        Command::Import { path } => platform::import(port, path).await,
        Command::Save(_) | Command::Export { .. } | Command::ExportBatch { .. } => {
            anyhow::bail!("Settings and export work must use the maintenance dispatcher")
        }
    }
}

fn fail(port: &Port, error: anyhow::Error) {
    let mut state = port.state.lock().unwrap();
    state.diagnose(error);
    state.snapshot.message = "The operation stopped. Review the diagnostic before retrying.".into();
}

async fn active(
    port: &Port,
    receiver: &mut mpsc::Receiver<Command>,
    command: Command,
) -> Result<()> {
    let operation = execute(port, command);
    tokio::pin!(operation);
    loop {
        tokio::select! {
            biased;
            result = &mut operation => return result,
            Some(command) = receiver.recv() => {
                if maintenance(port, command).is_some() { anyhow::bail!("Unexpected concurrent VPN command"); }
            }
        }
    }
}

fn maintenance(port: &Port, command: Command) -> Option<Command> {
    let (result, saving) = match command {
        Command::Save(preferences) => (platform::save(port, preferences), true),
        Command::Export { path, destination } => (platform::export(port, path, destination), false),
        Command::ExportBatch { items, directory } => {
            (crate::export::batch(port, items, &directory), false)
        }
        other => return Some(other),
    };
    let mut state = port.state.lock().unwrap();
    if let Err(error) = result {
        state.diagnose(error);
    }
    if saving {
        state.snapshot.saving = false;
    } else {
        state.snapshot.exporting = false;
    }
    None
}

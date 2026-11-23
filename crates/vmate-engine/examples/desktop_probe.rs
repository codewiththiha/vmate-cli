//! Runner probe of the same public engine port consumed by Slint.

use anyhow::{Context, Result, bail};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vmate_engine::{Configuration, Engine, model::Command};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 || !["recent", "scan"].contains(&args[4].as_str()) {
        bail!("usage: desktop_probe DATABASE CONFIG_DIR OPENVPN recent|scan");
    }
    std::fs::create_dir_all(&args[2])?;
    let engine = Engine::start(Configuration {
        database: PathBuf::from(&args[1]),
        config_dir: PathBuf::from(&args[2]),
        openvpn: PathBuf::from(&args[3]),
    })?;
    let port = engine.port();
    let deadline = Instant::now() + Duration::from_secs(180);
    while port.snapshot().events.is_empty() && Instant::now() < deadline {
        if !port.snapshot().error.is_empty() {
            bail!("{}", port.snapshot().error);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if args[4] == "scan" {
        port.send(Command::Scan)?;
        while port.snapshot().busy && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let state = port.snapshot();
    if state.busy || Instant::now() >= deadline {
        bail!("Engine probe exceeded its deadline");
    }
    if !state.error.is_empty() {
        bail!("{}", state.error);
    }
    let relay = state
        .relays
        .first()
        .context("No successful public relays; result is inconclusive")?;
    let export = PathBuf::from(&args[2]).join("engine-export.ovpn");
    port.send(Command::Export {
        path: relay.path.clone(),
        destination: export.clone(),
    })?;
    while port.snapshot().busy && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if port.snapshot().busy || !port.snapshot().error.is_empty() {
        bail!("Export failed or timed out");
    }
    if std::fs::read(&export)? != std::fs::read(&relay.path)? {
        bail!("Export bytes differ");
    }
    println!(
        "{}",
        serde_json::json!({"mode": args[4], "tested": state.completed,
        "saved_relays": state.relays.len(), "export_verified": true, "tunnel_tested": false})
    );
    Ok(())
}

//! Real engine ownership tests with local shell fixtures and isolated storage.

use std::{
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use vmate_engine::{Configuration, Engine, Port, model::Command};

fn wait(port: &Port, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(condition(), "engine state: {:?}", port.snapshot());
}

fn start_engine(script: &str) -> (tempfile::TempDir, Engine, Port) {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("openvpn");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'OpenVPN fixture'; else {script}; fi\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engine = Engine::start(Configuration {
        config_dir: dir.path().join("config"),
        database: dir.path().join("history.db"),
        openvpn: binary,
    })
    .unwrap();
    let port = engine.port();
    wait(&port, || !port.snapshot().events.is_empty());
    (dir, engine, port)
}

#[test]
fn scan_and_export_use_engine_ports_without_a_gui() {
    let (dir, engine, port) = start_engine("echo 'Initialization Sequence Completed'");
    port.send(Command::Scan).unwrap();
    wait(&port, || !port.snapshot().busy);
    assert!(
        port.snapshot().error.is_empty(),
        "{}",
        port.snapshot().error
    );
    assert_eq!(port.snapshot().completed, 277);
    assert_eq!(port.snapshot().relays.len(), 277);
    let destination = dir.path().join("export.ovpn");
    port.send(Command::Export {
        path: port.snapshot().relays[0].path.clone(),
        destination: destination.clone(),
    })
    .unwrap();
    wait(&port, || !port.snapshot().busy);
    assert!(destination.exists());
    drop(engine);
}

#[test]
fn immediate_cancellation_is_not_lost_before_worker_dispatch() {
    let (_dir, engine, port) = start_engine("sleep 30");
    port.send(Command::Scan).unwrap();
    port.cancel();
    wait(&port, || !port.snapshot().busy);
    assert!(!port.snapshot().connected);
    assert!(port.snapshot().completed < 277);
    drop(engine);
}

#[test]
fn teardown_joins_a_running_scan_before_engine_drop_returns() {
    let (_dir, engine, port) = start_engine("sleep 30");
    port.send(Command::Scan).unwrap();
    wait(&port, || port.snapshot().total == 277);
    let start = Instant::now();
    drop(engine);
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(!port.snapshot().busy);
}

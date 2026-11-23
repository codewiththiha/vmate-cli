//! Exercise the real CLI and desktop engine concurrently against shared storage.

use assert_cmd::Command;
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};
use vmate_engine::{Configuration, Engine, Port, model::Command as Intent};

fn wait(port: &Port, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(condition(), "{:?}", port.snapshot());
}

fn cli(database: &Path, binary: &Path) -> Command {
    let mut command = Command::cargo_bin("vmate-cli").unwrap();
    command
        .env("VMATE_DB", database)
        .env("VMATE_OPENVPN_BIN", binary)
        .env("VMATE_NO_ELEVATE", "1");
    command
}

#[test]
fn cli_writes_appear_in_running_desktop_and_engine_writes_in_cli() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("history.db");
    let binary = directory.path().join("openvpn");
    std::fs::write(&binary, "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'OpenVPN parity fixture'; else echo 'Initialization Sequence Completed'; fi\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engine = Engine::start(Configuration {
        config_dir: directory.path().join("config"),
        database: database.clone(),
        openvpn: binary.clone(),
    })
    .unwrap();
    let port = engine.port();
    wait(&port, || !port.snapshot().events.is_empty());
    let inputs = directory.path().join("inputs");
    std::fs::create_dir(&inputs).unwrap();
    let config = inputs.join("cli_jp.ovpn");
    std::fs::write(
        &config,
        "client\ndev tun\nremote cli-parity.example 1194 udp\n",
    )
    .unwrap();
    let config = config.canonicalize().unwrap();
    cli(&database, &binary)
        .args(["scan", inputs.to_str().unwrap()])
        .assert()
        .success();
    wait(&port, || {
        port.snapshot().relays.iter().any(|r| r.path == config)
    });
    assert!(
        port.snapshot().error.is_empty(),
        "{}",
        port.snapshot().error
    );
    port.send(Intent::Scan).unwrap();
    wait(&port, || !port.snapshot().busy);
    assert!(
        port.snapshot().error.is_empty(),
        "{}",
        port.snapshot().error
    );
    let saved = port.snapshot().relays[0].path.clone();
    let recent = cli(&database, &binary)
        .args(["recent", "--no-tui", "--all"])
        .output()
        .unwrap();
    assert!(recent.status.success());
    assert!(
        String::from_utf8_lossy(&recent.stdout)
            .contains(saved.file_name().unwrap().to_str().unwrap())
    );
    let exported = directory.path().join("exports");
    cli(&database, &binary)
        .args([
            "recent",
            "--no-tui",
            "--all",
            "--export",
            exported.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(std::fs::read_dir(&exported).unwrap().next().is_some());
    drop(engine);
}

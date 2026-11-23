//! Local setup failures preserve relay history and abort further probing.

use assert_cmd::Command;
use predicates::prelude::*;
use std::os::unix::fs::PermissionsExt;

fn binary() -> Command {
    let mut command = Command::cargo_bin("vmate-cli").unwrap();
    command.env("VMATE_NO_ELEVATE", "1");
    command
}

#[test]
fn local_tun_error_does_not_delete_a_successful_config() {
    let dir = tempfile::tempdir().unwrap();
    let configs = dir.path().join("configs");
    std::fs::create_dir(&configs).unwrap();
    let config = configs.join("vpngate_jp.ovpn");
    std::fs::write(&config, "client\nremote 127.0.0.1 1194\ndev tun\n").unwrap();
    let fake = dir.path().join("openvpn");
    std::fs::write(
        &fake,
        "#!/bin/sh\necho 'Initialization Sequence Completed'\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let db = dir.path().join("db");
    binary()
        .args([
            "--db",
            db.to_str().unwrap(),
            "--openvpn-bin",
            fake.to_str().unwrap(),
            "scan",
            configs.to_str().unwrap(),
        ])
        .assert()
        .success();
    std::fs::write(
        &fake,
        "#!/bin/sh\necho 'Cannot open TUN/TAP dev: Permission denied'\n",
    )
    .unwrap();
    binary()
        .args([
            "--db",
            db.to_str().unwrap(),
            "--openvpn-bin",
            fake.to_str().unwrap(),
            "connect",
            "--no-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("relay history preserved"));
    binary()
        .args(["--db", db.to_str().unwrap(), "recent", "--no-tui"])
        .assert()
        .success()
        .stdout(predicate::str::contains("vpngate_jp.ovpn"));
}

#[test]
fn missing_openvpn_aborts_scan_instead_of_recording_remote_failures() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("jp.ovpn"), "remote 127.0.0.1 1194\n").unwrap();
    binary()
        .args([
            "--db",
            dir.path().join("db").to_str().unwrap(),
            "--openvpn-bin",
            "/nonexistent-vmate-openvpn",
            "scan",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to spawn"));
}

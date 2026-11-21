//! Connect command integration tests (no real VPN required).

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn tmp_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("vmate.db");
    (dir, db)
}

#[test]
fn connect_empty_history_suggests_scan() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["connect"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("No connectable configs"))
        .stdout(predicate::str::contains("vmate-cli scan"));
}

#[test]
fn connect_with_filter_empty_history_mentions_filter_and_scan() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["connect", "--filter", "jp"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No connectable configs matched filter: JP",
        ))
        .stdout(predicate::str::contains("vmate-cli scan"));
}

#[test]
fn connect_strict_filter_rejects_non_matching_explicit_config() {
    let (dir, db) = tmp_db();
    let config = dir.path().join("us-config.ovpn");
    std::fs::write(&config, "client\nremote us.example.com 1194 udp\n").unwrap();

    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args([
        "connect",
        config.to_str().unwrap(),
        "--filter",
        "jp",
        "--strict-filter",
    ])
    .env("VMATE_DB", &db)
    .env("VMATE_NO_ELEVATE", "1")
    .env("IPINFO_TOKEN", "unused") // not needed; filename has no country code
    .assert()
    .failure()
    .stderr(predicate::str::contains("does not match filter"));
}

#[test]
fn piped_noninteractive_connection_never_requires_a_terminal() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, db) = tmp_db();
    let config = dir.path().join("vpngate_jp.ovpn");
    std::fs::write(&config, "client\nremote 127.0.0.1 1194\n").unwrap();
    let fake = dir.path().join("fake-openvpn");
    std::fs::write(
        &fake,
        "#!/bin/sh\necho 'Initialization Sequence Completed'\nsleep 0.05\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut command = Command::cargo_bin("vmate-cli").unwrap();
    command
        .args([
            "--no-elevate",
            "--db",
            db.to_str().unwrap(),
            "--openvpn-bin",
            fake.to_str().unwrap(),
            "connect",
            config.to_str().unwrap(),
            "--no-interactive",
        ])
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .success()
        .stderr(predicate::str::contains("Connected successfully"));
}

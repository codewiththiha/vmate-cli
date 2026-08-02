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
fn connect_empty_history_is_graceful() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["connect", "--no-killall"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains("No connectable configs"));
}

#[test]
fn connect_with_filter_empty_history_mentions_filter() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["connect", "--filter", "jp", "--no-killall"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No connectable configs matched filter: JP",
        ));
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
        "--no-killall",
    ])
    .env("VMATE_DB", &db)
    .env("VMATE_NO_ELEVATE", "1")
    .env("IPINFO_TOKEN", "unused") // not needed; filename has no country code
    .assert()
    .failure()
    .stderr(predicate::str::contains("does not match filter"));
}

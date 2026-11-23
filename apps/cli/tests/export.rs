//! `--export` integration tests for `scan` (fresh matches) and `recent`
//! (previously scanned configs). Uses a fake `openvpn` that reports success
//! immediately, like the scan tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};

fn tmp_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("vmate.db");
    (dir, db)
}

/// Write an executable fake `openvpn` that prints the success banner.
fn write_fake_openvpn(dir: &Path) -> PathBuf {
    let fake = dir.join("fake-openvpn.sh");
    std::fs::write(
        &fake,
        "#!/bin/sh\nprintf 'Initialization Sequence Completed\\n'\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    fake
}

/// Seed the DB by scanning `configs` with the fake openvpn.
fn scan_configs(dir: &Path, configs: &Path) -> PathBuf {
    let db = dir.join("vmate.db");
    let fake = write_fake_openvpn(dir);
    let mut scan = Command::cargo_bin("vmate-cli").unwrap();
    scan.args(["scan", configs.to_str().unwrap()])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .env("VMATE_OPENVPN_BIN", fake.to_str().unwrap())
        .assert()
        .success();
    db
}

fn write_jp_kr(configs: &Path) {
    std::fs::create_dir_all(configs).unwrap();
    std::fs::write(
        configs.join("vpngate_jp.ovpn"),
        "client\nremote jp.example.com 1194 udp\ndev tun\n",
    )
    .unwrap();
    std::fs::write(
        configs.join("vpngate_kr.ovpn"),
        "client\nremote kr.example.com 1194 udp\ndev tun\n",
    )
    .unwrap();
}

#[test]
fn scan_export_copies_fresh_matches_and_updates_recent() {
    let (dir, db) = tmp_db();
    let fake = write_fake_openvpn(dir.path());
    let configs = dir.path().join("configs");
    write_jp_kr(&configs);
    let out = dir.path().join("exported");

    let mut scan = Command::cargo_bin("vmate-cli").unwrap();
    scan.args([
        "scan",
        configs.to_str().unwrap(),
        "--filter",
        "jp",
        "--export",
        out.to_str().unwrap(),
    ])
    .env("VMATE_DB", &db)
    .env("VMATE_NO_ELEVATE", "1")
    .env("VMATE_OPENVPN_BIN", fake.to_str().unwrap())
    .assert()
    .success()
    .stdout(predicate::str::contains("Exported 1 of 1 configs"));

    // Only the freshly-scanned JP match is exported, with a country prefix.
    assert!(out.join("JP_vpngate_jp.ovpn").exists());
    assert!(!out.join("JP_vpngate_kr.ovpn").exists());

    // The scan still stored its successes, so `recent` shows the configs.
    let mut recent = Command::cargo_bin("vmate-cli").unwrap();
    recent
        .args(["recent", "--no-tui"])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains("vpngate_jp.ovpn"))
        .stdout(predicate::str::contains("vpngate_kr.ovpn"));
}

#[test]
fn recent_export_copies_stored_configs() {
    let (dir, _db) = tmp_db();
    let configs = dir.path().join("configs");
    write_jp_kr(&configs);
    let db = scan_configs(dir.path(), &configs);

    let out = dir.path().join("exported");
    let mut recent = Command::cargo_bin("vmate-cli").unwrap();
    recent
        .args([
            "recent",
            "--no-tui",
            "--filter",
            "jp",
            "--export",
            out.to_str().unwrap(),
        ])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported 1 of 1 configs"));

    assert!(out.join("JP_vpngate_jp.ovpn").exists());
    assert!(!out.join("JP_vpngate_kr.ovpn").exists());
}

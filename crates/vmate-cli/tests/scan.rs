//! Scan command integration tests.
//!
//! These avoid real OpenVPN by pointing at empty directories or by using a
//! tiny fake `openvpn` script that prints the success banner.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn tmp_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("vmate.db");
    (dir, db)
}

#[test]
fn scan_empty_dir_succeeds() {
    let (dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate").unwrap();
    cmd.args([
        "scan",
        dir.path().to_str().unwrap(),
        "--no-save",
        "--no-killall",
    ])
    .env("VMATE_DB", &db)
    .env("VMATE_NO_ELEVATE", "1")
    .assert()
    .success()
    .stdout(predicate::str::contains("Found matched: 0"));
}

#[test]
fn scan_missing_dir_fails_cleanly() {
    let (_, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate").unwrap();
    cmd.args(["scan", "/nonexistent/vmate-test-dir", "--no-killall"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("directory does not exist"));
}

/// A fake `openvpn` that reports success immediately, letting us exercise the
/// full scan -> store -> report path without a real VPN.
#[test]
fn scan_stores_and_reports_successes() {
    let (dir, db) = tmp_db();

    let fake = dir.path().join("fake-openvpn.sh");
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

    let configs = dir.path().join("configs");
    std::fs::create_dir_all(&configs).unwrap();
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

    let mut scan = Command::cargo_bin("vmate").unwrap();
    scan.args(["scan", configs.to_str().unwrap(), "--no-killall"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .env("VMATE_OPENVPN_BIN", fake.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Found matched: 2"))
        .stdout(predicate::str::contains("Found total:   2"));

    // The successes must now be visible via `recent`.
    let mut recent = Command::cargo_bin("vmate").unwrap();
    recent
        .args(["recent", "--no-tui"])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains("vpngate_jp.ovpn"))
        .stdout(predicate::str::contains("vpngate_kr.ovpn"));
}

/// With a filter, only matching countries are reported even though more
/// configs succeed.
#[test]
fn scan_filter_limits_reported_matches() {
    let (dir, db) = tmp_db();

    let fake = dir.path().join("fake-openvpn.sh");
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

    let configs = dir.path().join("configs");
    std::fs::create_dir_all(&configs).unwrap();
    for (name, country) in [
        ("vpngate_jp.ovpn", "jp"),
        ("vpngate_kr.ovpn", "kr"),
        ("vpngate_us.ovpn", "us"),
    ] {
        std::fs::write(
            configs.join(name),
            format!("client\nremote {country}.example.com 1194 udp\ndev tun\n"),
        )
        .unwrap();
    }

    let mut scan = Command::cargo_bin("vmate").unwrap();
    scan.args([
        "scan",
        configs.to_str().unwrap(),
        "--filter",
        "jp",
        "--no-killall",
    ])
    .env("VMATE_DB", &db)
    .env("VMATE_NO_ELEVATE", "1")
    .env("VMATE_OPENVPN_BIN", fake.to_str().unwrap())
    .assert()
    .success()
    .stdout(predicate::str::contains("Found matched: 1"))
    .stdout(predicate::str::contains("Found total:   3"))
    .stdout(predicate::str::contains("vpngate_jp.ovpn"))
    .stdout(predicate::str::contains("vpngate_us.ovpn").not());
}

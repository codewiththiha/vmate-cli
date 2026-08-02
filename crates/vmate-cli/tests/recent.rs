//! Recent command integration tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;

fn tmp_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("vmate.db");
    (dir, db)
}

#[test]
fn recent_empty_db_is_graceful() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["recent"])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains("No successful configs"));
}

#[test]
fn recent_empty_db_with_filter_mentions_filter() {
    let (_dir, db) = tmp_db();
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["recent", "--filter", "kr"])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No successful configs matched filter: KR",
        ));
}

#[test]
fn recent_plain_table_works_with_seeded_db() {
    let (dir, db) = tmp_db();

    // Seed the DB through the public CLI by running a scan against a fake
    // openvpn (mirrors tests/scan.rs).
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
        "client\nremote jp.example.com 1194 udp\n",
    )
    .unwrap();

    Command::cargo_bin("vmate-cli")
        .unwrap()
        .args(["scan", configs.to_str().unwrap(), "--no-killall"])
        .env("VMATE_DB", &db)
        .env("VMATE_NO_ELEVATE", "1")
        .env("VMATE_OPENVPN_BIN", fake.to_str().unwrap())
        .assert()
        .success();

    let mut recent = Command::cargo_bin("vmate-cli").unwrap();
    recent
        .args(["recent", "--no-tui", "--filter", "jp"])
        .env("VMATE_DB", &db)
        .assert()
        .success()
        .stdout(predicate::str::contains("JP"))
        .stdout(predicate::str::contains("vpngate_jp.ovpn"));
}

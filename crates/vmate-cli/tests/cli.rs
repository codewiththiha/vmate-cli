//! CLI smoke tests: help, version, completions and flag validation.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("OpenVPN config scanner"));
}

#[test]
fn version_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.arg("--version").assert().success();
}

#[test]
fn scan_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["scan", "--help"]).assert().success();
}

#[test]
fn connect_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["connect", "--help"]).assert().success();
}

#[test]
fn recent_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["recent", "--help"]).assert().success();
}

#[test]
fn all_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["all", "--help"]).assert().success();
}

#[test]
fn export_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["export", "--help"]).assert().success();
}

#[test]
fn doctor_help_works() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["doctor", "--help"]).assert().success();
}

#[test]
fn completions_generate_bash() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn invalid_filter_is_rejected() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    cmd.args(["recent", "--filter", "JPNX"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid country code"));
}

#[test]
fn repeated_filter_flags_work() {
    let mut cmd = Command::cargo_bin("vmate-cli").unwrap();
    // Parsing succeeds; the DB just has nothing yet.
    cmd.args(["recent", "-f", "jp", "-f", "kr"])
        .env(
            "VMATE_DB",
            tempfile::tempdir().unwrap().path().join("vmate.db"),
        )
        .assert()
        .success();
}

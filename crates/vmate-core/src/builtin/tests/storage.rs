//! Regression checks for sudo-created and read-only generated configs.

use super::super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn matching_read_only_config_is_reused_without_a_write() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = &enumerate(Provider::VpnGate, Proto::Udp)[0];
    let path = materialize(cfg, dir.path()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let before = std::fs::metadata(&path).unwrap();
    assert_eq!(materialize(cfg, dir.path()).unwrap(), path);
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
}

#[test]
fn stale_read_only_config_is_replaced_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = &enumerate(Provider::VpnGate, Proto::Udp)[0];
    let path = materialize(cfg, dir.path()).unwrap();
    std::fs::write(&path, "old generated config").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let inode = std::fs::metadata(&path).unwrap().ino();
    materialize(cfg, dir.path()).unwrap();
    assert_ne!(std::fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(std::fs::read_to_string(path).unwrap(), build_config(cfg));
}

#[test]
fn generated_file_symlink_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = &enumerate(Provider::VpnGate, Proto::Udp)[0];
    let path = materialize(cfg, dir.path()).unwrap();
    let target = dir.path().join("do-not-change");
    std::fs::write(&target, "original").unwrap();
    std::fs::remove_file(&path).unwrap();
    symlink(&target, &path).unwrap();
    assert!(materialize(cfg, dir.path()).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "original");
}

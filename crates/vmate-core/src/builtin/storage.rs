//! Materialization of generated built-in configs.

use super::{BuiltinConfig, build_config, remote_host_port};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Write the built config to `<dir>/<host>-<port>.ovpn`, creating the directory
/// if needed, and return the path.
pub fn materialize(cfg: &BuiltinConfig, dir: &Path) -> Result<PathBuf> {
    let (host, port) = remote_host_port(&cfg.remote)
        .with_context(|| format!("invalid remote line: {}", cfg.remote))?;
    std::fs::create_dir_all(dir)
        .with_context(|| format!("cannot create directory {}", dir.display()))?;
    let path = dir.join(format!("{host}-{port}.ovpn"));
    std::fs::write(&path, build_config(cfg))
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}


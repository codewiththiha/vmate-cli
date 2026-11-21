//! Materialize generated configs without rewriting matching read-only files.

use super::{BuiltinConfig, build_config, remote_host_port};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Reuse matching configs or atomically replace stale generated content.
pub fn materialize(cfg: &BuiltinConfig, dir: &Path) -> Result<PathBuf> {
    let (host, port) = remote_host_port(&cfg.remote)
        .with_context(|| format!("invalid remote line: {}", cfg.remote))?;
    std::fs::create_dir_all(dir)
        .with_context(|| format!("cannot create directory {}", dir.display()))?;
    let path = dir.join(format!("{host}-{port}.ovpn"));
    repair_generated_directories(dir)?;
    reject_symlink(&path)?;
    let content = build_config(cfg);
    if std::fs::read(&path).ok().as_deref() == Some(content.as_bytes()) {
        crate::system::repair_ownership(&path);
        return Ok(path);
    }
    let write = || -> Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(content.as_bytes())?;
        file.persist(&path)?;
        crate::system::repair_ownership(&path);
        Ok(())
    };
    write().with_context(|| {
        format!(
            "cannot update generated config {}; previous sudo runs may own its directory. \
             Repair once with: sudo chown -R \"$(id -u):$(id -g)\" \"{}\"",
            path.display(),
            dir.display()
        )
    })?;
    Ok(path)
}

fn repair_generated_directories(dir: &Path) -> Result<()> {
    let root = crate::paths::builtin_dir()?;
    if !dir.starts_with(&root) {
        return Ok(());
    }
    for path in dir.ancestors().take_while(|path| path.starts_with(&root)) {
        reject_symlink(path)?;
        crate::system::repair_ownership(path);
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("refusing generated-config symlink: {}", path.display());
    }
    Ok(())
}

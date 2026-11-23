//! Resolve explicit overrides, private bundle payloads, then system OpenVPN.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Never download executables or change permissions while resolving a binary.
pub fn resolve(override_bin: Option<&str>) -> Result<PathBuf> {
    if let Some(value) = override_bin.filter(|value| *value != "openvpn") {
        return Ok(value.into());
    }
    if let Some(value) = std::env::var_os("VMATE_OPENVPN_BIN") {
        return Ok(value.into());
    }
    let executable = std::env::current_exe()?;
    resolve_from(&executable)
}

fn resolve_from(executable: &Path) -> Result<PathBuf> {
    let parent = executable.parent().unwrap_or(Path::new("."));
    for relative in [
        "../Resources/openvpn/bin/openvpn",
        "../lib/vmate/openvpn/bin/openvpn",
    ] {
        let path = parent.join(relative);
        if path.exists() {
            if !path.is_file() {
                bail!("bundled OpenVPN is not a file: {}", path.display());
            }
            return Ok(path);
        }
    }
    Ok("openvpn".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_private_payload_before_path() {
        let dir = tempfile::tempdir().unwrap();
        let payload = dir.path().join("lib/vmate/openvpn/bin");
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::create_dir_all(&payload).unwrap();
        std::fs::write(payload.join("openvpn"), "fixture").unwrap();
        let found = resolve_from(&dir.path().join("bin/vmate-cli")).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            payload.join("openvpn").canonicalize().unwrap()
        );
    }
}

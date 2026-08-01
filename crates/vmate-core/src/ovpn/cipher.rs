//! Repair of outdated cipher directives in `.ovpn` files.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

pub const OLD_CIPHER: &str = "cipher AES-128-CBC";
pub const NEW_CIPHER: &str = "data-ciphers AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305:AES-128-CBC";
const MODIFIED_MARKER: &str = "#MODIFIED";

/// Outcome of a cipher modification pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifyOutcome {
    /// The cipher line was replaced and the file written.
    Modified,
    /// The file already starts with `#MODIFIED`.
    AlreadyModified,
    /// The file does not contain the old cipher line.
    NoChangeNeeded,
}

/// Replace `cipher AES-128-CBC` with a modern `data-ciphers` line and prepend
/// `#MODIFIED` so the file is not processed twice. Writes atomically and
/// preserves CRLF line endings when present.
pub fn modify_config_cipher(path: &Path) -> Result<ModifyOutcome> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;

    if content.starts_with("#MODIFIED\n") || content.starts_with("#MODIFIED\r\n") {
        return Ok(ModifyOutcome::AlreadyModified);
    }

    let crlf = content.contains("\r\n");
    let lines: Vec<&str> = content
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut modified = false;
    for line in &lines {
        if line.trim().contains(OLD_CIPHER) {
            out_lines.push(NEW_CIPHER.to_string());
            modified = true;
        } else {
            out_lines.push(line.to_string());
        }
    }

    if !modified {
        return Ok(ModifyOutcome::NoChangeNeeded);
    }

    let sep = if crlf { "\r\n" } else { "\n" };
    let mut out = format!("{MODIFIED_MARKER}{sep}");
    out.push_str(&out_lines.join(sep));
    out.push_str(sep);

    atomic_write(path, out.as_bytes())?;
    Ok(ModifyOutcome::Modified)
}

/// Write bytes to `path` via a temp file + rename so a crash never leaves a
/// half-written config.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("cannot create temp file in {}", parent.display()))?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    tmp.persist(path)
        .map(|_| ())
        .map_err(|e| e.error)
        .with_context(|| format!("cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(body: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f
    }

    #[test]
    fn replaces_old_cipher() {
        let f = write_config("client\ncipher AES-128-CBC\ndev tun\n");
        assert_eq!(
            modify_config_cipher(f.path()).unwrap(),
            ModifyOutcome::Modified
        );
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.starts_with("#MODIFIED\n"));
        assert!(content.contains(NEW_CIPHER));
        assert!(!content.contains(OLD_CIPHER));
        assert!(content.contains("dev tun"));
    }

    #[test]
    fn does_not_modify_twice() {
        let f = write_config("#MODIFIED\ndev tun\n");
        assert_eq!(
            modify_config_cipher(f.path()).unwrap(),
            ModifyOutcome::AlreadyModified
        );
    }

    #[test]
    fn no_change_when_cipher_absent() {
        let f = write_config("dev tun\n");
        assert_eq!(
            modify_config_cipher(f.path()).unwrap(),
            ModifyOutcome::NoChangeNeeded
        );
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), "dev tun\n");
    }

    #[test]
    fn preserves_crlf() {
        let f = write_config("client\r\ncipher AES-128-CBC\r\ndev tun\r\n");
        modify_config_cipher(f.path()).unwrap();
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.starts_with("#MODIFIED\r\n"));
        assert!(content.contains("dev tun\r\n"));
    }
}

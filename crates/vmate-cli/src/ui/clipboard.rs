//! Clipboard copy with OSC 52 fallback.

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::fmt;

/// Which mechanism actually copied the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMethod {
    System,
    Osc52,
}

impl fmt::Display for ClipboardMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardMethod::System => write!(f, "system"),
            ClipboardMethod::Osc52 => write!(f, "OSC 52"),
        }
    }
}

/// Copy text to the system clipboard, falling back to the OSC 52 escape
/// sequence when no system clipboard is available.
pub fn copy_to_clipboard(text: &str) -> Result<ClipboardMethod> {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => {
            clipboard
                .set_text(text.to_string())
                .context("system clipboard is unavailable")?;
            Ok(ClipboardMethod::System)
        }
        Err(err) => {
            tracing::warn!(error = %err, "system clipboard unavailable; falling back to OSC 52");
            copy_with_osc52(text)?;
            Ok(ClipboardMethod::Osc52)
        }
    }
}

/// Copy via the OSC 52 terminal escape sequence.
///
/// Written to `/dev/tty` so it works even when stdout is redirected.
pub fn copy_with_osc52(text: &str) -> Result<()> {
    let encoded = STANDARD.encode(text);
    let payload = format!("\x1b]52;c;{encoded}\x07");

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut tty = OpenOptions::new()
            .write(true)
            .open("/dev/tty")
            .context("cannot open /dev/tty for OSC 52 copy")?;
        tty.write_all(payload.as_bytes())?;
        tty.flush()?;
    }

    Ok(())
}

//! Interactive scan cancellation without blocking the async worker pool.

use crate::ui::input::InputReader;
use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::IsTerminal;
use tokio_util::sync::CancellationToken;

/// Own scan key polling and restore terminal mode before returning to connect.
pub(crate) struct ScanInput {
    input: Option<InputReader>,
    raw: bool,
}

impl ScanInput {
    pub(crate) fn new(cancel: CancellationToken) -> Result<Self> {
        let raw = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if raw {
            enable_raw_mode()?;
        }
        match InputReader::new(raw, cancel) {
            Ok(input) => Ok(Self {
                input: Some(input),
                raw,
            }),
            Err(error) => {
                if raw {
                    let _ = disable_raw_mode();
                }
                Err(error)
            }
        }
    }
}

impl Drop for ScanInput {
    fn drop(&mut self) {
        drop(self.input.take());
        if self.raw {
            let _ = disable_raw_mode();
        }
    }
}

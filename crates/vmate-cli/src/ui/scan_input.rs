//! Interactive scan cancellation without blocking the async worker pool.

use crate::ui::input::InputReader;
use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::IsTerminal;
use tokio_util::sync::CancellationToken;
use vmate_core::connect::UserCommand;

/// Own scan key polling and restore terminal mode before returning to connect.
pub(crate) struct ScanInput {
    task: tokio::task::JoinHandle<()>,
    raw: bool,
}

impl ScanInput {
    pub(crate) async fn finish(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }

    pub(crate) fn new(cancel: CancellationToken) -> Result<Self> {
        let raw = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if raw {
            enable_raw_mode()?;
        }
        let mut input = match InputReader::new(raw) {
            Ok(input) => input,
            Err(error) => {
                if raw {
                    let _ = disable_raw_mode();
                }
                return Err(error);
            }
        };
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    command = input.next() => {
                        if command == Some(UserCommand::Quit) {
                            cancel.cancel();
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self { task, raw })
    }
}

impl Drop for ScanInput {
    fn drop(&mut self) {
        self.task.abort();
        if self.raw {
            let _ = disable_raw_mode();
        }
    }
}

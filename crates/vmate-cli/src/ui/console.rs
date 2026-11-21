//! Plain connection reporting without a terminal or keyboard polling.

use anyhow::Result;
use async_trait::async_trait;
use vmate_core::connect::{ConnectHost, ConnectionStatus, UserCommand};

pub(super) struct ConsoleHost {
    pub(super) verbose: bool,
}

#[async_trait]
impl ConnectHost for ConsoleHost {
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()> {
        eprintln!("{}", status.message);
        Ok(())
    }

    async fn notify(&mut self, message: &str) -> Result<()> {
        eprintln!("{message}");
        Ok(())
    }

    async fn log(&mut self, line: &str) -> Result<()> {
        if self.verbose {
            eprintln!("{line}");
        }
        Ok(())
    }

    async fn copy(&mut self, text: &str) -> Result<()> {
        eprintln!("{text}");
        Ok(())
    }

    async fn poll_command(&mut self) -> Option<UserCommand> {
        std::future::pending().await
    }

    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

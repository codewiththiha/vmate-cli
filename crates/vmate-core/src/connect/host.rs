//! The UI surface the connect service drives.

use crate::connect::state::ConnectionStatus;
use anyhow::Result;
use async_trait::async_trait;

/// A user command produced by the interactive UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCommand {
    Next,
    Reconnect,
    CopyPath,
    ToggleVerbose,
    Help,
    Quit,
}

/// The UI surface the connect service drives.
#[async_trait]
pub trait ConnectHost: Send {
    /// Render a state change.
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()>;
    /// Show a transient message.
    async fn notify(&mut self, message: &str) -> Result<()>;
    /// Show a verbose OpenVPN output line.
    async fn log(&mut self, line: &str) -> Result<()>;
    /// Copy text to the clipboard.
    async fn copy(&mut self, text: &str) -> Result<()>;
    /// Poll for a key without blocking indefinitely (re-renders meanwhile).
    async fn poll_command(&mut self) -> Option<UserCommand>;
    /// Restore the terminal when the session ends.
    async fn finish(&mut self) -> Result<()>;
}

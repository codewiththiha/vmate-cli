//! Managed-session events projected into frontend-neutral state.

use crate::Port;
use anyhow::Result;
use async_trait::async_trait;
use vmate_core::connect::{ConnectHost, ConnectionStatus, UserCommand};

pub(super) struct Host(pub Port);

#[async_trait]
impl ConnectHost for Host {
    async fn status(&mut self, status: &ConnectionStatus) -> Result<()> {
        let mut state = self.0.state.lock().unwrap();
        state.snapshot.connected = status.connected;
        state.snapshot.status = if status.connected {
            "Connected"
        } else {
            "Connecting"
        }
        .into();
        state.snapshot.message = status.message.clone();
        state.event(status.message.clone());
        Ok(())
    }
    async fn log(&mut self, line: &str) -> Result<()> {
        self.0
            .state
            .lock()
            .unwrap()
            .event(line.chars().take(2048).collect());
        Ok(())
    }
    async fn notify(&mut self, message: &str) -> Result<()> {
        self.log(message).await
    }
    async fn copy(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
    async fn poll_command(&mut self) -> Option<UserCommand> {
        std::future::pending().await
    }
    async fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

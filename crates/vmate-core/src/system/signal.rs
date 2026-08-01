//! Graceful shutdown signals.

/// Why the application is shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    CtrlC,
    Term,
}

/// Wait for Ctrl+C or SIGTERM.
pub async fn shutdown_signal() -> ShutdownReason {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sig) => sig,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return ShutdownReason::CtrlC;
            }
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => ShutdownReason::CtrlC,
            _ = sigterm.recv() => ShutdownReason::Term,
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        ShutdownReason::CtrlC
    }
}

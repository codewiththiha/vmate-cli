//! Graceful shutdown signals.

/// Why the application is shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    CtrlC,
    Term,
    Hangup,
}

/// Wait for Ctrl+C, SIGTERM, or terminal hangup.
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

        let mut hangup = signal(SignalKind::hangup()).ok();
        tokio::select! {
            _ = async {
                if let Some(signal) = &mut hangup { signal.recv().await; }
                else { std::future::pending::<()>().await; }
            } => ShutdownReason::Hangup,
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

/// Own a shutdown listener until the surrounding operation ends.
pub struct ShutdownTask(tokio::task::JoinHandle<()>);

impl Drop for ShutdownTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Cancel an operation on SIGINT or SIGTERM without bypassing its teardown.
pub fn cancel_on_shutdown(cancel: tokio_util::sync::CancellationToken) -> ShutdownTask {
    ShutdownTask(tokio::spawn(async move {
        let _ = shutdown_signal().await;
        cancel.cancel();
    }))
}

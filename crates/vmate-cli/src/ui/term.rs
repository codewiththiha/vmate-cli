//! Terminal helpers.

use std::io::IsTerminal;

/// Whether stdout is attached to a real terminal.
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Put the terminal back the way it was found: raw mode off, alternate screen
/// left, mouse capture disabled.
///
/// Safe to call more than once and on a terminal that was never touched, so it
/// can be used both by the RAII guard below and by the emergency signal path.
pub fn restore_terminal() {
    use crossterm::event::DisableMouseCapture;
    use crossterm::execute;
    use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

/// RAII guard that restores the terminal when dropped.
///
/// Used by the interactive TUIs so that raw mode and the alternate screen are
/// restored even on panic or error paths.
pub struct TuiGuard;

impl Drop for TuiGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Keeps an emergency shutdown handler alive for one interactive session.
///
/// Raw mode turns Ctrl+C into a key event, so this only sees a real signal:
/// `--no-interactive` runs, or a SIGINT/SIGTERM that arrives before the TUI
/// takes over. Without it such a signal would kill vmate mid-session, leaving
/// the terminal in raw mode and the tunnel process behind.
pub struct SignalGuard(Option<tokio::task::JoinHandle<()>>);

impl SignalGuard {
    /// Install the handler for the processes registered in `registry`.
    pub fn install(registry: std::sync::Arc<vmate_core::system::ProcessRegistry>) -> Self {
        let handle = tokio::spawn(async move {
            let reason = vmate_core::system::shutdown_signal().await;
            registry.kill_all_immediate();
            restore_terminal();
            // 130 is the shell convention for SIGINT, 143 for SIGTERM.
            std::process::exit(match reason {
                vmate_core::system::ShutdownReason::CtrlC => 130,
                vmate_core::system::ShutdownReason::Term => 143,
            });
        });
        Self(Some(handle))
    }
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

//! Terminal helpers.

use std::io::IsTerminal;

/// Whether stdout is attached to a real terminal.
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// RAII guard that restores the terminal when dropped.
///
/// Used by the interactive TUIs so that raw mode and the alternate screen are
/// restored even on panic or error paths.
pub struct TuiGuard;

impl Drop for TuiGuard {
    fn drop(&mut self) {
        use crossterm::event::DisableMouseCapture;
        use crossterm::execute;
        use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

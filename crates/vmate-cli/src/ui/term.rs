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

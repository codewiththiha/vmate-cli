//! Classification of OpenVPN output lines.

use crate::ovpn::keywords::ERROR_KEYWORDS;

/// What a single OpenVPN log line means for us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnLineClass {
    /// `Initialization Sequence Completed` — the tunnel is up.
    Success,
    /// One of [`ERROR_KEYWORDS`] was found.
    Error(&'static str),
    /// OpenVPN is pausing before a restart.
    RestartPause,
    /// Anything else.
    Info,
}

/// Classify a single trimmed log line.
pub fn classify_line(line: &str) -> VpnLineClass {
    let trimmed = line.trim();

    if trimmed.contains("Initialization Sequence Completed") {
        return VpnLineClass::Success;
    }

    if trimmed.contains("Restart pause") {
        return VpnLineClass::RestartPause;
    }

    for keyword in ERROR_KEYWORDS {
        if trimmed.contains(keyword) {
            return VpnLineClass::Error(keyword);
        }
    }

    VpnLineClass::Info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_success() {
        assert_eq!(
            classify_line("Initialization Sequence Completed"),
            VpnLineClass::Success
        );
        assert_eq!(
            classify_line("  Initialization Sequence Completed  "),
            VpnLineClass::Success
        );
    }

    #[test]
    fn recognizes_restart_pause() {
        assert_eq!(
            classify_line("Restart pause, 5 second(s)"),
            VpnLineClass::RestartPause
        );
    }

    #[test]
    fn recognizes_errors() {
        for kw in ERROR_KEYWORDS {
            assert!(matches!(
                classify_line(&format!("error: {kw}")),
                VpnLineClass::Error(_)
            ));
        }
    }

    #[test]
    fn treats_other_lines_as_info() {
        assert_eq!(classify_line("Initial packet from ..."), VpnLineClass::Info);
        assert_eq!(classify_line(""), VpnLineClass::Info);
        assert_eq!(classify_line("   "), VpnLineClass::Info);
    }

    #[test]
    fn success_wins_over_error_keywords() {
        // "process exiting" appears inside the success banner on some versions;
        // success must take priority.
        assert_eq!(
            classify_line("Initialization Sequence Completed (process exiting)"),
            VpnLineClass::Success
        );
    }
}

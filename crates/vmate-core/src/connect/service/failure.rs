//! Display reasons for unsuccessful remote handshakes.

use crate::ovpn::process::ConnectOutcome;

pub(super) fn reason(bad: ConnectOutcome) -> String {
    match bad {
        ConnectOutcome::Error(m) => format!("connection error: {m}"),
        ConnectOutcome::RestartPause => "restart pause".to_string(),
        ConnectOutcome::TimedOut => "connection timed out".to_string(),
        ConnectOutcome::Exited => "openvpn exited before connecting".to_string(),
        ConnectOutcome::LocalError(_)
        | ConnectOutcome::Connected
        | ConnectOutcome::Cancelled
        | ConnectOutcome::Next => {
            unreachable!()
        }
    }
}

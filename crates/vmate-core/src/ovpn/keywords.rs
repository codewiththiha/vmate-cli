//! Output signatures used to classify OpenVPN behaviour.

/// Lines that indicate a connection failure, ported from the Go original.
pub const ERROR_KEYWORDS: &[&str] = &[
    "No route to host",
    "TLS key negotiation failed",
    "Connection timed out",
    "Connection refused",
    "AUTH_FAILED",
    "Network unreachable",
    "Host is down",
    "Name or service not known",
    "VERIFY ERROR",
    "certificate verify failed",
    "Inactivity timeout",
    "Ping timeout",
    "Cannot open TUN/TAP dev",
    "write to TUN/TAP: Input/output error",
    "read: Connection reset by peer",
    "handshake failure",
    "fatal error",
    "process exiting",
    "killed",
];

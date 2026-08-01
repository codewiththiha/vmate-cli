//! Connection state shared between the service and the UI.

use crate::connect::queue::Candidate;

/// The current state of the connect session, rendered by the UI host.
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub candidate: Option<Candidate>,
    pub message: String,
    pub filter: String,
}

//! Connect orchestration: candidate queue, session state, host contract and
//! the service that drives the connect loop.

mod host;
pub mod queue;
pub mod service;
mod session;
pub mod state;

pub use host::{ConnectHost, UserCommand};
pub use queue::{Candidate, ConnectQueue};
pub use service::{ConnectOptions, ConnectService};
pub use state::ConnectionStatus;

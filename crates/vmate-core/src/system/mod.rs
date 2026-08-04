//! Operating-system integration: process killing, root detection, signals.

pub mod killer;
pub mod root;
pub mod signal;

pub use killer::{
    CleanupGuard, ProcessKiller, ProcessRegistry, RealProcessKiller, force_kill_process_group,
    kill_process_group, kill_process_tree_graceful, killall_openvpn,
};
pub use root::{elevate_with_sudo, is_root, require_root_for};
pub use signal::{ShutdownReason, shutdown_signal};

//! Operating-system integration: process killing, root detection, signals.

pub mod killer;
pub mod root;
pub mod signal;

pub use killer::{
    CleanupGuard, ProcessKiller, ProcessRegistry, RealProcessKiller, SWITCH_GRACE,
    force_kill_process_group, kill_process_group, kill_process_tree_graceful,
    kill_process_tree_with_grace, killall_openvpn,
};
pub use root::{
    carried_environment, elevate_with_sudo, elevate_without_prompt, interactive, is_elevated,
    is_root, repair_ownership, require_root_for, root_summary, sudo_can_elevate_without_prompt,
    sudo_uid_gid, sudo_user,
};
pub use signal::{ShutdownReason, shutdown_signal};

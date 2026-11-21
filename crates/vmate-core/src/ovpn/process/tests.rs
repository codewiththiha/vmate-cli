use super::*;
use crate::system::killer::{CleanupGuard, RealProcessKiller};

#[tokio::test]
async fn normal_teardown_unregisters_pid() {
    let registry = ProcessRegistry::new();
    let mut handle = spawn_openvpn("sh", &["-c".into(), "sleep 30".into()], &registry).unwrap();
    assert_eq!(registry.registered().len(), 1);
    handle
        .kill_graceful(&RealProcessKiller {
            killall_enabled: false,
        })
        .await;
    assert!(registry.registered().is_empty());
}

#[tokio::test]
async fn unexpected_drop_kills_and_unregisters() {
    let registry = ProcessRegistry::new();
    let handle = spawn_openvpn("sh", &["-c".into(), "sleep 30".into()], &registry).unwrap();
    let pid = handle.child.id().unwrap();
    drop(handle);
    assert!(registry.registered().is_empty());
    tokio::time::timeout(Duration::from_secs(2), async {
        while nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("dropped child must be reaped");
}

#[tokio::test]
async fn guard_cleans_targeted_children_without_killall() {
    let registry = Arc::new(ProcessRegistry::new());
    let mut handle = spawn_openvpn("sh", &["-c".into(), "sleep 30".into()], &registry).unwrap();
    let guard = CleanupGuard::new(
        Arc::new(RealProcessKiller {
            killall_enabled: false,
        }),
        registry.clone(),
        false,
    );
    drop(guard);
    tokio::time::timeout(Duration::from_secs(2), handle.child.wait())
        .await
        .expect("targeted cleanup must run")
        .unwrap();
    assert!(registry.registered().is_empty());
}

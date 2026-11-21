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

#[tokio::test]
async fn pushed_options_warning_does_not_abort_a_successful_probe() {
    let (tx, mut rx) = mpsc::channel(4);
    tx.send(
        "Options error: Unrecognized option in [PUSH-OPTIONS]: block-outside-dns (2.6.14)".into(),
    )
    .await
    .unwrap();
    tx.send("Initialization Sequence Completed".into())
        .await
        .unwrap();
    let outcome = monitor_test(&mut rx, Duration::from_secs(1), CancellationToken::new()).await;
    assert_eq!(outcome, MonitorOutcome::Success);
}

#[tokio::test]
async fn startup_options_error_keeps_diagnostic_details() {
    let (tx, mut rx) = mpsc::channel(1);
    let diagnostic = "Options error: Unrecognized option in config.ovpn:4: data-ciphers (2.4.12)";
    tx.send(diagnostic.into()).await.unwrap();
    let outcome = monitor_test(&mut rx, Duration::from_secs(1), CancellationToken::new()).await;
    assert_eq!(outcome, MonitorOutcome::LocalError(diagnostic.into()));
}

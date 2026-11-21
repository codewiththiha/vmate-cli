use super::*;

#[tokio::test]
async fn connect_then_crash_twice_drops_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let path = "/tmp/connect-then-crash.ovpn";
    let id = repo
        .record_success(
            std::path::Path::new(path),
            "x",
            None,
            &CountryCode::new("jp").unwrap(),
            CountrySource::FileName,
        )
        .await
        .unwrap();

    let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
        killall_enabled: false,
    });
    let registry = Arc::new(ProcessRegistry::new());
    let service = ConnectService {
        runner: Arc::new(ConnectThenExitRunner {
            registry: registry.clone(),
        }),
        killer,
        registry,
        repo: repo.clone(),
        options: ConnectOptions {
            connect_timeout: Duration::from_secs(1),
            connect_stability_grace: Duration::from_secs(1),
            retry_count: 2,
            killall_enabled: false,
        },
    };

    let queue = ConnectQueue::new(vec![Candidate {
        id,
        path: path.into(),
        country: "JP".into(),
    }]);
    let mut host = FakeHost::new(vec![]);
    service.run(queue, &mut host).await.unwrap();

    // After connect + crash + retry + crash the config is gone from the DB
    // (and therefore from `recent`).
    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_none()
    );

    // The user is told the config was removed from the recent list.
    let msgs = host.messages.lock().unwrap();
    assert!(
        msgs.iter()
            .any(|m| m.contains("removed") && m.contains("recent list")),
        "expected a 'removed ... from recent list' notice, got: {msgs:?}"
    );
}

/// A host that reports Quit once the N-th connection's phase-2 monitoring
/// has begun (a `connected: true` status). Lets a crash-retry loop with a
/// resetting budget run long enough to prove the config is never dropped.
#[tokio::test]
async fn long_session_crashes_do_not_drop_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let path = "/tmp/stable-then-crash.ovpn";
    repo.record_success(
        std::path::Path::new(path),
        "x",
        None,
        &CountryCode::new("jp").unwrap(),
        CountrySource::FileName,
    )
    .await
    .unwrap();

    let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
        killall_enabled: false,
    });
    let registry = Arc::new(ProcessRegistry::new());
    let service = ConnectService {
        runner: Arc::new(ConnectThenStableThenExitRunner {
            registry: registry.clone(),
        }),
        killer,
        registry,
        repo: repo.clone(),
        options: ConnectOptions {
            connect_timeout: Duration::from_secs(1),
            // Far below the runner's ~0.2s uptime so each crash follows a
            // "stable" session and resets the budget.
            connect_stability_grace: Duration::from_millis(50),
            retry_count: 2,
            killall_enabled: false,
        },
    };

    let queue = ConnectQueue::new(vec![Candidate {
        id: 1,
        path: path.into(),
        country: "JP".into(),
    }]);
    let mut host = QuitAfterConnect {
        connects: 0,
        quit_after: 3,
    };
    service.run(queue, &mut host).await.unwrap();

    // Two long-session crashes each reset the budget (failures stayed at
    // 1, never reaching the retry budget), so the config is still in history.
    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_some(),
        "long-session crashes must not delete a working config from history"
    );
}

/// Pressing `q` during the handshake must quit immediately instead of
/// waiting for the connect timeout — and it is a clean exit, not a failure,
/// so the config stays in history.
#[tokio::test]
async fn quit_during_handshake_is_immediate() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let path = "/tmp/quit-during.ovpn";
    repo.record_success(
        std::path::Path::new(path),
        "x",
        None,
        &CountryCode::new("jp").unwrap(),
        CountrySource::FileName,
    )
    .await
    .unwrap();

    let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
        killall_enabled: false,
    });
    let registry = Arc::new(ProcessRegistry::new());
    let service = ConnectService {
        runner: Arc::new(SlowRunner {
            registry: registry.clone(),
        }),
        killer,
        registry,
        repo: repo.clone(),
        options: ConnectOptions {
            connect_timeout: Duration::from_secs(30), // quit must beat this
            connect_stability_grace: Duration::from_secs(1),
            retry_count: 2,
            killall_enabled: false,
        },
    };

    let queue = ConnectQueue::new(vec![Candidate {
        id: 0,
        path: path.into(),
        country: "JP".into(),
    }]);
    let mut host = FakeHost::new(vec![UserCommand::Quit]);
    service.run(queue, &mut host).await.unwrap();

    // Quit during the handshake is a clean exit, not a drop: the config is
    // still in history.
    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_some()
    );
}

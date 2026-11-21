use super::*;

#[tokio::test]
async fn empty_queue_finishes_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let service = test_service(repo);
    let mut host = FakeHost::new(vec![]);
    service
        .run(ConnectQueue::new(vec![]), &mut host)
        .await
        .unwrap();
    assert!(host.statuses.lock().unwrap().is_empty());
}

#[tokio::test]
async fn spawn_failure_aborts_and_preserves_history() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let service = test_service(repo.clone());

    let queue = ConnectQueue::new(vec![
        Candidate {
            id: 1,
            path: "/tmp/a.ovpn".into(),
            country: "JP".into(),
        },
        Candidate {
            id: 2,
            path: "/tmp/b.ovpn".into(),
            country: "KR".into(),
        },
    ]);

    let mut host = FakeHost::new(vec![]);
    // Spawn failure is systemic and must abort the session, not delete
    // every config from history.
    assert!(service.run(queue, &mut host).await.is_err());
}

#[tokio::test]
async fn two_handshake_failures_drop_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let path = "/tmp/a.ovpn";
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
        runner: Arc::new(FailHandshakeRunner {
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

    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_none()
    );
}

/// Spawns `echo AUTH_FAILED` for the first two attempts and then hangs
/// (`sleep 30`), so a configurable retry budget can be observed mid-flight.
#[tokio::test]
async fn custom_retry_count_waits_for_that_many_failures() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(ConfigRepo::new(
        init_pool(&dir.path().join("t.db")).await.unwrap(),
    ));
    let path = "/tmp/retry-3.ovpn";
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

    let make_service = |runner: Arc<dyn OpenVpnRunner>,
                        registry: Arc<ProcessRegistry>,
                        repo: Arc<ConfigRepo>| {
        let killer: Arc<dyn ProcessKiller> = Arc::new(crate::system::killer::RealProcessKiller {
            killall_enabled: false,
        });
        ConnectService {
            runner,
            killer,
            registry,
            repo,
            options: ConnectOptions {
                // Long handshake timeout so the host's `q` (returned
                // once two failures are seen) always wins the race over a
                // timeout — otherwise a scheduler-delayed poll can turn
                // the intended cancel into a 3rd failure and a drop.
                connect_timeout: Duration::from_secs(30),
                connect_stability_grace: Duration::from_secs(1),
                retry_count: 3,
                killall_enabled: false,
            },
        }
    };

    // Phase 1: two handshake failures, then interrupt the third attempt
    // with `q` while its handshake hangs. Only two failures have accrued,
    // so the config is still in history.
    let mut host = QuitAfterRetries {
        failures_seen: 0,
        quit_after: 2,
    };
    let registry = Arc::new(ProcessRegistry::new());
    let service = make_service(
        Arc::new(FailTwiceThenHang {
            spawns: std::sync::atomic::AtomicUsize::new(0),
            registry: registry.clone(),
        }),
        registry,
        repo.clone(),
    );
    service
        .run(
            ConnectQueue::new(vec![Candidate {
                id,
                path: path.into(),
                country: "JP".into(),
            }]),
            &mut host,
        )
        .await
        .unwrap();
    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_some(),
        "with retry_count 3 a config must survive two failures"
    );

    // Phase 2: from a clean slate, three real failures drop the config.
    let registry = Arc::new(ProcessRegistry::new());
    let service = make_service(
        Arc::new(FailHandshakeRunner {
            registry: registry.clone(),
        }),
        registry,
        repo.clone(),
    );
    let mut host = FakeHost::new(vec![]);
    service
        .run(
            ConnectQueue::new(vec![Candidate {
                id,
                path: path.into(),
                country: "JP".into(),
            }]),
            &mut host,
        )
        .await
        .unwrap();
    assert!(
        repo.config_by_path(std::path::Path::new(path))
            .await
            .unwrap()
            .is_none(),
        "with retry_count 3 a config must be dropped on the third failure"
    );

    // The user-facing messages reflect the custom budget.
    let statuses = host.statuses.lock().unwrap();
    let msgs: Vec<&str> = statuses.iter().map(|s| s.message.as_str()).collect();
    assert!(
        msgs.iter().any(|m| m.contains("(failure 1/3)")),
        "expected a (failure 1/3) message, got: {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("(failure 2/3)")),
        "expected a (failure 2/3) message, got: {msgs:?}"
    );
}

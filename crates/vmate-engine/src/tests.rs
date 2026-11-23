//! Engine validation and bounded memory contracts without network access.

use super::*;

#[test]
fn preference_limits_reject_unbounded_work() {
    let mut preferences = model::Preferences::default();
    assert!(validate_preferences(&preferences).is_ok());
    preferences.workers = 501;
    assert!(validate_preferences(&preferences).is_err());
    preferences.workers = 500;
    preferences.timeout = 0;
    assert!(validate_preferences(&preferences).is_err());
}

#[test]
fn events_keep_only_the_newest_128_entries() {
    let mut state = State {
        snapshot: Snapshot::default(),
        operation: CancellationToken::new(),
    };
    for index in 0..1000 {
        state.event(index.to_string());
    }
    assert_eq!(state.snapshot.events.len(), 128);
    assert_eq!(state.snapshot.events[0], "872");
    assert_eq!(state.snapshot.events[127], "999");
}

#[test]
fn cancellation_bypasses_a_full_command_queue() {
    let (sender, _receiver) = mpsc::channel(1);
    let operation = CancellationToken::new();
    let port = Port {
        sender,
        shutdown: CancellationToken::new(),
        configuration: Arc::new(Configuration {
            config_dir: "/tmp".into(),
            database: "/tmp/test.db".into(),
            openvpn: "openvpn".into(),
        }),
        state: Arc::new(Mutex::new(State {
            snapshot: Snapshot::default(),
            operation: operation.clone(),
        })),
    };
    port.send(Command::Scan).unwrap();
    assert!(port.send(Command::Refresh).is_err());
    port.cancel();
    assert!(port.state.lock().unwrap().operation.is_cancelled());
}

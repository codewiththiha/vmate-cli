//! Engine validation and bounded memory contracts without network access.

use super::*;

#[test]
fn preference_limits_reject_unbounded_work() {
    let mut preferences = model::Preferences::default();
    assert!(validate_preferences(&preferences).is_ok());
    preferences.presets[0].workers = 501;
    assert!(validate_preferences(&preferences).is_err());
    preferences.presets[0].workers = 500;
    preferences.presets[0].timeout = 0;
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

#[test]
fn presets_require_unique_names_valid_assignments_and_bounded_loops() {
    let mut preferences = model::Preferences {
        retry_limit: 0,
        ..model::Preferences::default()
    };
    assert!(validate_preferences(&preferences).is_err());
    preferences.retry_limit = 21;
    assert!(validate_preferences(&preferences).is_err());
    preferences.retry_limit = 2;
    preferences.connect_preset = "Missing".into();
    assert!(validate_preferences(&preferences).is_err());
    preferences.connect_preset = "Default".into();
    preferences.presets.push(model::ScanPreset::default());
    assert!(validate_preferences(&preferences).is_err());
    preferences.presets[1].name = "Quick".into();
    preferences.scan_preset = "Quick".into();
    assert!(validate_preferences(&preferences).is_ok());
    let json = serde_json::to_vec(&preferences).unwrap();
    let loaded: model::Preferences = serde_json::from_slice(&json).unwrap();
    assert_eq!(loaded.scan_preset, "Quick");
    assert_eq!(loaded.connect_preset, "Default");
    assert_eq!(loaded.retry_limit, 2);
}

#[test]
fn tray_is_opt_in_and_round_trips_without_breaking_old_preferences() {
    let old: model::Preferences = serde_json::from_str("{}").unwrap();
    assert!(!old.tray_enabled);
    let enabled = model::Preferences {
        tray_enabled: true,
        ..old
    };
    let loaded: model::Preferences =
        serde_json::from_slice(&serde_json::to_vec(&enabled).unwrap()).unwrap();
    assert!(loaded.tray_enabled);
}

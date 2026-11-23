//! Exercise settings drafts, selection gestures and export previews.

use super::*;
use crate::binding;
use slint::Model;
use std::time::{Duration, Instant};
use vmate_engine::{
    Configuration, Engine,
    model::{Relay, Snapshot},
};

fn tick() {
    std::thread::sleep(Duration::from_millis(120));
    slint::platform::update_timers_and_animations();
}

pub(super) fn check(
    app: &AppWindow,
    window: &MinimalSoftwareWindow,
    pixels: &mut [slint::Rgb8Pixel],
) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("desktop.json"),
        r#"{"country_lookup":false}"#,
    )
    .unwrap();
    let engine = Engine::start(Configuration {
        config_dir: dir.path().into(),
        database: dir.path().join("history.db"),
        openvpn: "/bin/true".into(),
    })
    .unwrap();
    let port = engine.port();
    let timer = binding::bind(app, port.clone());
    let deadline = Instant::now() + Duration::from_secs(15);
    while !port.snapshot().ready && Instant::now() < deadline {
        tick();
    }
    tick();
    app.set_page(4);
    app.set_settings_page(1);
    app.set_preset_name("Renamed".into());
    tick();
    assert!(app.get_settings_dirty());
    assert!(app.get_settings_valid());
    app.invoke_new_preset();
    assert_eq!(app.get_editing_preset(), 1);
    app.set_preset_name("Quick".into());
    app.set_workers("12".into());
    app.set_timeout("4".into());
    app.set_scan_index(1);
    app.set_connect_index(1);
    app.invoke_select_preset(0);
    assert_eq!(app.get_preset_name(), "Renamed");
    app.invoke_select_preset(1);
    assert_eq!(app.get_preset_name(), "Quick");
    assert_eq!(app.get_workers(), "12");
    app.invoke_save();
    while (port.snapshot().saving || app.get_settings_dirty()) && Instant::now() < deadline {
        tick();
    }
    assert!(!app.get_settings_dirty(), "{}", app.get_settings_feedback());
    assert_eq!(port.snapshot().preferences.presets.len(), 2);
    assert_eq!(port.snapshot().preferences.connect_preset, "Quick");
    app.set_workers("0".into());
    tick();
    assert!(!app.get_settings_valid());
    app.invoke_discard_settings();
    tick();
    assert_eq!(app.get_workers(), "64");
    assert!(!app.get_settings_dirty());
    timer.stop();
    selection(app, window, pixels);
    drop(engine);
}

fn selection(app: &AppWindow, window: &MinimalSoftwareWindow, pixels: &mut [slint::Rgb8Pixel]) {
    app.set_page(0);
    app.set_query("".into());
    app.set_selection_mode(false);
    binding::apply(
        app,
        &Snapshot {
            relays: vec![
                Relay {
                    path: "/tmp/a.ovpn".into(),
                    name: "relay".into(),
                    country: "JP".into(),
                    ..Default::default()
                },
                Relay {
                    path: "/tmp/b.ovpn".into(),
                    name: "second".into(),
                    country: "US".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    );
    app.window().request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(pixels, 1160);
    });
    let position = LogicalPosition::new(400.0, 285.0);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    std::thread::sleep(Duration::from_millis(650));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        app.get_selected_count(),
        1,
        "Long press must select the relay"
    );
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    assert_eq!(
        app.get_selected_count(),
        1,
        "Release must not toggle the held row again"
    );
    app.invoke_select_all_relays(false);
    click_button(app, position, PointerEventButton::Right);
    click_button(
        app,
        LogicalPosition::new(440.0, 301.0),
        PointerEventButton::Left,
    );
    assert_eq!(
        app.get_selected_count(),
        1,
        "Right-click Select must mark the relay"
    );
    click_button(
        app,
        LogicalPosition::new(230.0, 172.0),
        PointerEventButton::Left,
    );
    assert_eq!(
        app.get_selected_count(),
        2,
        "Select all button must select visible rows"
    );
    app.invoke_select_all_relays(true);
    assert_eq!(app.get_selected_count(), 2);
    app.invoke_export_relay();
    assert!(app.get_export_open());
    assert_eq!(app.get_export_rows().row_count(), 2);
    preview_styles(app);
}

fn preview_styles(app: &AppWindow) {
    app.set_export_style(2);
    app.set_export_full_country(false);
    app.invoke_preview_export();
    assert!(app.get_export_preview().starts_with("relay_JP.ovpn"));
    app.set_export_full_country(true);
    tick();
    assert!(app.get_export_preview().starts_with("relay_japan.ovpn"));
    app.set_export_style(1);
    tick();
    assert!(app.get_export_preview().starts_with("japan.ovpn"));
    app.set_export_style(0);
    tick();
    assert!(app.get_export_preview().starts_with("relay.ovpn"));
    app.set_export_open(false);
    app.invoke_select_all_relays(false);
    assert_eq!(app.get_selected_count(), 0);
}

fn click_button(app: &AppWindow, position: LogicalPosition, button: PointerEventButton) {
    app.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    app.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
}

//! Real frontend binding through the actor and shared scan/storage implementation.

use crate::{AppWindow, binding};
use slint::{ComponentHandle, Model, platform::software_renderer::MinimalSoftwareWindow};
use std::{
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use vmate_engine::{Configuration, Engine};

pub(super) fn check(
    app: &AppWindow,
    window: &MinimalSoftwareWindow,
    pixels: &mut [slint::Rgb8Pixel],
    click: &dyn Fn(f32, f32),
) {
    let selected = app.get_relays().row_data(0).unwrap();
    app.set_selected(0);
    let mut snapshot = vmate_engine::model::Snapshot {
        relays: vec![
            vmate_engine::model::Relay::default(),
            vmate_engine::model::Relay {
                path: selected.path.to_string().into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    binding::apply(app, &snapshot);
    assert_eq!(
        app.get_selected(),
        1,
        "external inserts must preserve selected path identity"
    );
    snapshot.relays.pop();
    binding::apply(app, &snapshot);
    assert_eq!(
        app.get_selected(),
        -1,
        "external removal must clear selection"
    );
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("openvpn");
    std::fs::write(&binary,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'OpenVPN UI fixture'; else echo 'Initialization Sequence Completed'; fi\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let engine = Engine::start(Configuration {
        config_dir: directory.path().join("config"),
        database: directory.path().join("history.db"),
        openvpn: binary,
    })
    .unwrap();
    let port = engine.port();
    let _binding = binding::bind(app, port.clone());
    let deadline = Instant::now() + Duration::from_secs(20);
    while port.snapshot().events.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!port.snapshot().events.is_empty());
    for mac in [false, true] {
        app.set_mac(mac);
        exercise_scan(app, &port, window, pixels, click);
    }
    drop(engine);
}

fn exercise_scan(
    app: &AppWindow,
    port: &vmate_engine::Port,
    window: &MinimalSoftwareWindow,
    pixels: &mut [slint::Rgb8Pixel],
    click: &dyn Fn(f32, f32),
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    app.set_page(0);
    app.set_diagnostic("".into());
    app.set_busy(false);
    app.set_connected(false);
    app.window().request_redraw();
    slint::platform::update_timers_and_animations();
    window.draw_if_needed(|renderer| {
        renderer.render(pixels, 1160);
    });
    click(1080.0, 77.0);
    assert!(
        port.snapshot().busy,
        "actual Slint callback must submit a scan to the actor"
    );
    while (port.snapshot().busy || app.get_relays().row_count() != 277) && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
        slint::platform::update_timers_and_animations();
    }
    assert!(!port.snapshot().busy, "scan must finish off the GUI thread");
    assert!(
        port.snapshot().error.is_empty(),
        "{}",
        port.snapshot().error
    );
    assert_eq!(
        app.get_relays().row_count(),
        277,
        "actor results must reach the real Slint model"
    );
}

pub(super) fn brand_alignment(pixels: &[slint::Rgb8Pixel]) {
    let bounds = |left: usize, right: usize| {
        let rows: Vec<usize> = (65..96)
            .filter(|y| {
                (left..right).any(|x| {
                    let pixel = pixels[y * 1160 + x];
                    pixel.r < 90 && pixel.g < 110 && pixel.b < 100
                })
            })
            .collect();
        assert!(!rows.is_empty(), "branding region must contain visible ink");
        rows[0] + rows[rows.len() - 1]
    };
    assert!(
        bounds(30, 60).abs_diff(bounds(69, 154)) <= 4,
        "logo and wordmark visual centers must align within two pixels"
    );
}

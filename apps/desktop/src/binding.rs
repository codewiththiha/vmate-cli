//! Convert engine DTOs and user intent, never perform VPN orchestration here.

use crate::{AppWindow, ProfileRow, RelayRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::time::Duration;
mod actions;

use vmate_engine::{Port, model::Snapshot};

pub(super) fn bind(app: &AppWindow, port: Port) -> slint::Timer {
    app.set_mac(cfg!(target_os = "macos"));
    let weak = app.as_weak();
    app.on_drag_window(move || {
        use slint::winit_030::WinitWindowAccessor;
        if let Some(app) = weak.upgrade() {
            app.window().with_winit_window(|window| {
                let _ = window.drag_window();
            });
        }
    });
    app.set_install_command(setup_instructions().into());
    actions::bind(app, &port);
    let p = port.clone();
    let weak = app.as_weak();
    app.on_filter_relays(move || {
        if let Some(app) = weak.upgrade() {
            apply(&app, &p.snapshot());
        }
    });
    let closing = actions::closing(app, &port);
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    let mut previous = String::new();
    let mut preferences = String::new();
    let mut first_ready = true;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let snapshot = port.snapshot();
            if first_ready && !snapshot.events.is_empty() {
                if snapshot.openvpn.contains("unavailable")
                    || snapshot.system_service.contains("unavailable")
                    || (snapshot.relays.is_empty() && snapshot.profiles.is_empty())
                {
                    app.set_page(2);
                }
                first_ready = false;
            }
            if (closing.get() || port.is_shutting_down()) && !snapshot.busy {
                let _ = app.hide();
                return;
            }
            let preference_key = format!("{:?}", snapshot.preferences);
            if preference_key != preferences {
                app.set_workers(snapshot.preferences.workers.to_string().into());
                app.set_timeout(snapshot.preferences.timeout.to_string().into());
                app.set_reduced(snapshot.preferences.reduced_motion);
                app.set_lookup(snapshot.preferences.country_lookup);
                preferences = preference_key;
            }
            let signature = format!("{:?}", snapshot);
            if signature != previous {
                apply(&app, &snapshot);
                previous = signature;
            }
        },
    );
    timer
}

pub(super) fn apply(app: &AppWindow, snapshot: &Snapshot) {
    app.set_busy(snapshot.busy);
    app.set_connected(snapshot.connected);
    app.set_status(snapshot.status.clone().into());
    app.set_message(snapshot.message.clone().into());
    app.set_diagnostic(snapshot.error.clone().into());
    app.set_completed(snapshot.completed as i32);
    app.set_total(snapshot.total as i32);
    app.set_openvpn(snapshot.openvpn.clone().into());
    app.set_service(snapshot.system_service.clone().into());
    app.set_activity(snapshot.events.join("\n\n").into());
    app.set_relays(ModelRc::new(VecModel::from(
        snapshot
            .relays
            .iter()
            .filter(|r| {
                format!("{} {}", r.name, r.country)
                    .to_lowercase()
                    .contains(&app.get_query().to_lowercase())
            })
            .map(|r| RelayRow {
                name: r.name.clone().into(),
                country: r.country.clone().into(),
                checked: r.checked.clone().into(),
                path: r.path.to_string_lossy().to_string().into(),
            })
            .collect::<Vec<_>>(),
    )));
    app.set_profiles(ModelRc::new(VecModel::from(
        snapshot
            .profiles
            .iter()
            .map(|p| ProfileRow {
                name: p.name.clone().into(),
                uuid: p.uuid.clone().into(),
            })
            .collect::<Vec<_>>(),
    )));
}

fn setup_instructions() -> String {
    if cfg!(target_os = "macos") {
        return "Install Tunnelblick from https://tunnelblick.net\nOpen it once, then import a relay from vmate.".into();
    }
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    if os.contains("ID=arch") || os.contains("ID=manjaro") || os.contains("ID_LIKE=arch") {
        "sudo pacman -S networkmanager networkmanager-openvpn openvpn".into()
    } else if os.contains("ID=fedora") || os.contains("ID_LIKE=\"fedora") {
        "sudo dnf install NetworkManager-openvpn NetworkManager-openvpn-gnome openvpn".into()
    } else {
        "Debian / Ubuntu:\nsudo apt install network-manager-openvpn network-manager-openvpn-gnome openvpn\nOther distributions: install NetworkManager + its OpenVPN plugin.".into()
    }
}

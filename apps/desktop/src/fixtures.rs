//! Screenshot-only data; never written into the engine or user database.

use crate::{AppWindow, binding};
use vmate_engine::model::{Profile, Relay, Snapshot};

pub(super) const SCENARIOS: &[&str] = &[
    "preferences-bottom",
    "home-idle",
    "home-scanning",
    "home-connecting",
    "home-connected",
    "home-retrying",
    "home-stopping",
    "home-exhausted",
    "home-permission",
    "connected-library",
    "about",
    "library",
    "relay-details",
    "setup-bottom",
    "empty",
    "scanning",
    "connection",
    "connected",
    "setup-linux",
    "setup-macos",
    "missing-openvpn",
    "permission-denied",
    "activity",
    "preferences",
];

pub(super) fn apply(app: &AppWindow, scenario: &str) {
    let mut snapshot = Snapshot {
        openvpn: "OpenVPN 2.7.8 · private bundle executable".into(),
        system_service: "NetworkManager · OpenVPN plugin available".into(),
        relays: [
            (
                "public-vpn-249.opengw.net",
                "JP",
                "Today, 14:32 UTC · handshake passed",
            ),
            (
                "vpn126573544.opengw.net",
                "KR",
                "Today, 14:31 UTC · handshake passed",
            ),
            (
                "public-vpn-38.opengw.net",
                "US",
                "Today, 14:30 UTC · handshake passed",
            ),
            (
                "vpn610596269.opengw.net",
                "DE",
                "Today, 14:29 UTC · handshake passed",
            ),
            (
                "public-vpn-260.opengw.net",
                "UNKNOWN",
                "Today, 14:28 UTC · handshake passed",
            ),
        ]
        .iter()
        .map(|(name, country, checked)| Relay {
            name: (*name).into(),
            country: (*country).into(),
            checked: (*checked).into(),
            path: format!("/home/you/.config/vmate-cli/builtin/vpn-gate/udp/{name}-1194.ovpn")
                .into(),
        })
        .collect(),
        profiles: vec![Profile {
            name: "VPN Gate · Japan".into(),
            uuid: "01234567-89ab-cdef-0123-456789abcdef".into(),
        }],
        events: vec![
            "14:32:07  Scan complete: 277 tested, 34 handshakes passed.".into(),
            "14:32:07  All probe processes reaped. No tunnel created.".into(),
            "14:31:49  Testing VPN Gate built-in UDP configurations.".into(),
            "14:31:48  Using private bundled OpenVPN 2.7.8.".into(),
            "14:31:48  Country lookup disabled. No relay IPs sent to ipinfo.io.".into(),
        ],
        ..Snapshot::default()
    };
    app.set_review_scroll(false);
    app.set_query("".into());
    app.set_mac(false);
    app.set_page(0);
    app.set_selected(0);
    app.set_install_command(
        "sudo apt install network-manager-openvpn network-manager-openvpn-gnome openvpn".into(),
    );
    app.set_preset_names(slint::ModelRc::new(slint::VecModel::from(vec![
        "Default".into(),
    ])));
    app.set_preset_name("Default".into());
    app.set_retry_limit("3".into());
    if scenario.starts_with("home-") {
        home(app, scenario, &mut snapshot);
    }
    select_scenario(app, scenario, &mut snapshot);
    binding::apply(app, &snapshot);
}

fn select_scenario(app: &AppWindow, scenario: &str, snapshot: &mut Snapshot) {
    match scenario {
        "connected-library" => {
            snapshot.busy = true;
            snapshot.connected = true;
            snapshot.status = "Connected".into();
            snapshot.message =
                "A system VPN session is active. Disconnect in Connections before scanning.".into();
        }
        "about" => app.set_page(5),
        "relay-details" => app.set_review_scroll(true),
        "setup-bottom" => {
            app.set_page(2);
            app.set_review_scroll(true);
        }
        "empty" => {
            snapshot.relays.clear();
            app.set_selected(-1);
        }
        "scanning" => {
            snapshot.busy = true;
            snapshot.status = "Scanning".into();
            snapshot.completed = 148;
            snapshot.total = 277;
        }
        "connection" => app.set_page(1),
        "connected" => {
            app.set_page(1);
            snapshot.busy = true;
            snapshot.connected = true;
            snapshot.status = "Connected".into();
            snapshot.message = "Connected through NetworkManager · VPN Gate Japan".into();
        }
        "setup-linux" => app.set_page(2),
        "setup-macos" => {
            app.set_page(2);
            app.set_mac(true);
            app.set_install_command("Install Tunnelblick from https://tunnelblick.net\nOpen it once, then import a relay from vmate.".into());
            snapshot.system_service = "macOS · existing Tunnelblick installation".into();
        }
        "missing-openvpn" => {
            app.set_page(2);
            snapshot.openvpn = "OpenVPN not found on PATH or in the private bundle".into();
            snapshot.error = "OpenVPN is unavailable. Install the system package or choose the bundled edition, then check setup again.".into();
        }
        "permission-denied" => {
            app.set_page(1);
            snapshot.error = "NetworkManager: Not authorized to control networking. Check your desktop polkit agent or ask your administrator.".into();
        }
        "activity" => app.set_page(3),
        "preferences" => app.set_page(4),
        "preferences-bottom" => {
            app.set_page(4);
            app.set_review_scroll(true);
        }
        _ => {}
    }
}

fn home(app: &AppWindow, scenario: &str, snapshot: &mut Snapshot) {
    app.set_page(6);
    match scenario {
        "home-scanning" => {
            snapshot.busy = true;
            snapshot.status = "Scanning".into();
            snapshot.completed = 148;
            snapshot.total = 277;
        }
        "home-connecting" => {
            snapshot.busy = true;
            snapshot.status = "Connecting".into();
        }
        "home-connected" => {
            snapshot.busy = true;
            snapshot.connected = true;
            snapshot.status = "Connected".into();
        }
        "home-retrying" => {
            snapshot.busy = true;
            snapshot.status = "Retrying".into();
        }
        "home-stopping" => {
            snapshot.busy = true;
            snapshot.message = "Disconnecting and reaping owned processes…".into();
        }
        "home-exhausted" => {
            snapshot.error = "Retry limit reached. Connect to try again.".into();
        }
        "home-permission" => {
            snapshot.error =
                "NetworkManager: Not authorized. Start your desktop polkit agent, then try again."
                    .into();
        }
        _ => {}
    }
}

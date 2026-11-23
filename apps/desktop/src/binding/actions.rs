//! Translate explicit UI actions into bounded engine commands.

use crate::AppWindow;
use slint::{ComponentHandle, Model};
use vmate_engine::{Port, model::Command};

pub(super) fn bind(app: &AppWindow, port: &Port) {
    let p = port.clone();
    let weak = app.as_weak();
    app.on_scan(move || send(&p, &weak, Command::Scan));
    let p = port.clone();
    let weak = app.as_weak();
    app.on_auto_connect(move || send(&p, &weak, Command::AutoConnect));
    let p = port.clone();
    app.on_cancel(move || p.cancel());
    let p = port.clone();
    let weak = app.as_weak();
    app.on_refresh(move || send(&p, &weak, Command::Refresh));
    let p = port.clone();
    let weak = app.as_weak();
    app.on_connect_profile(move |index| {
        if let Some(app) = weak.upgrade()
            && let Some(profile) = app.get_profiles().row_data(index as usize)
        {
            send(&p, &weak, Command::Connect(profile.uuid.to_string()));
        }
    });
    let p = port.clone();
    let weak = app.as_weak();
    app.on_lookup_country(move |path| {
        send(
            &p,
            &weak,
            Command::LookupCountry {
                path: path.to_string().into(),
            },
        );
    });
    relay_actions(app, port);
    preferences(app, port);
}

fn relay_actions(app: &AppWindow, port: &Port) {
    let p = port.clone();
    let weak = app.as_weak();
    app.on_import_relay(move || {
        if let Some(app) = weak.upgrade()
            && let Some(row) = app.get_relays().row_data(app.get_selected() as usize)
        {
            send(
                &p,
                &weak,
                Command::Import {
                    path: row.path.to_string().into(),
                },
            );
        }
    });
    let p = port.clone();
    let weak = app.as_weak();
    app.on_export_relay(move || {
        let Some(app) = weak.upgrade() else { return };
        let Some(row) = app.get_relays().row_data(app.get_selected() as usize) else {
            return;
        };
        let p = p.clone();
        let target = weak.clone();
        if let Err(error) = slint::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_file_name("relay.ovpn")
                .save_file()
                .await
            {
                send(
                    &p,
                    &target,
                    Command::Export {
                        path: row.path.to_string().into(),
                        destination: file.path().into(),
                    },
                );
            }
        }) {
            app.set_diagnostic(format!("Cannot open export dialog: {error}").into());
        }
    });
}

fn preferences(app: &AppWindow, port: &Port) {
    super::settings::bind(app, port);
    let weak = app.as_weak();
    app.on_copy_command(move || {
        if let Some(app) = weak.upgrade() {
            match arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(app.get_install_command().to_string()))
            {
                Ok(()) => app.set_message(
                    "Setup instructions copied. Review before running in a terminal.".into(),
                ),
                Err(error) => app.set_diagnostic(format!("Clipboard unavailable: {error}").into()),
            }
        }
    });
}

fn send(port: &Port, app: &slint::Weak<AppWindow>, command: Command) {
    if let Err(error) = port.send(command)
        && let Some(app) = app.upgrade()
    {
        app.set_diagnostic(format!("{error:#}").into());
    }
}

pub(super) fn closing(app: &AppWindow, port: &Port) -> std::rc::Rc<std::cell::Cell<bool>> {
    let closing = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = closing.clone();
    let p = port.clone();
    let weak = app.as_weak();
    app.on_quit(move || {
        flag.set(true);
        p.cancel();
        if !p.snapshot().busy
            && let Some(app) = weak.upgrade()
        {
            let _ = app.hide();
            let _ = slint::quit_event_loop();
        }
    });
    let flag = closing.clone();
    let p = port.clone();
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if let Some(app) = weak.upgrade()
            && app.get_tray_active()
        {
            let _ = app.hide();
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        flag.set(true);
        p.cancel();
        if p.snapshot().busy {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            slint::CloseRequestResponse::HideWindow
        }
    });
    closing
}

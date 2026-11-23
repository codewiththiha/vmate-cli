//! Optional StatusNotifier ownership; window callbacks stay on Slint's thread.

use crate::AppWindow;
use ksni::blocking::TrayMethods;
use slint::ComponentHandle;
use std::sync::mpsc::{SyncSender, sync_channel};

pub(super) struct Session {
    sender: Option<SyncSender<bool>>,
    worker: Option<std::thread::JoinHandle<()>>,
    requested: bool,
}

impl Session {
    pub(super) fn new(app: slint::Weak<AppWindow>) -> Self {
        let (sender, receiver) = sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut handle: Option<ksni::blocking::Handle<Icon>> = None;
            while let Ok(enabled) = receiver.recv() {
                if let Some(previous) = handle.take() {
                    previous.shutdown().wait();
                }
                if enabled {
                    match (Icon { app: app.clone() }).spawn() {
                        Ok(active) => {
                            report(&app, true, None);
                            handle = Some(active);
                        }
                        Err(error) => report(&app, false, Some(error.to_string())),
                    }
                } else {
                    report(&app, false, None);
                }
            }
            if let Some(active) = handle {
                active.shutdown().wait();
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
            requested: false,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        if enabled != self.requested
            && self
                .sender
                .as_ref()
                .is_some_and(|sender| sender.try_send(enabled).is_ok())
        {
            self.requested = enabled;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Icon {
    app: slint::Weak<AppWindow>,
}

impl ksni::Tray for Icon {
    fn id(&self) -> String {
        "vmate".into()
    }

    fn title(&self) -> String {
        "vmate".into()
    }

    fn icon_name(&self) -> String {
        "vmate".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let image = image::load_from_memory(include_bytes!("../assets/tray.png"))
            .expect("embedded tray PNG is valid")
            .to_rgba8();
        let data = image
            .pixels()
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect();
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data,
        }]
    }

    fn activate(&mut self, _: i32, _: i32) {
        reopen(&self.app, false);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Open vmate".into(),
                activate: Box::new(|this: &mut Self| reopen(&this.app, false)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings".into(),
                activate: Box::new(|this: &mut Self| reopen(&this.app, true)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit vmate".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.app.upgrade_in_event_loop(|app| app.invoke_quit());
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        report(&self.app, true, None);
    }

    fn watcher_offline(&self, _: ksni::OfflineReason) -> bool {
        report(
            &self.app,
            false,
            Some("Desktop tray host is unavailable".into()),
        );
        true
    }
}

fn report(app: &slint::Weak<AppWindow>, active: bool, error: Option<String>) {
    let _ = app.upgrade_in_event_loop(move |app| {
        app.set_tray_active(active);
        if !active {
            let _ = app.show();
        }
        if let Some(error) = error {
            app.set_diagnostic(
                format!("System tray unavailable: {error}. Closing exits vmate.").into(),
            );
        }
    });
}

fn reopen(app: &slint::Weak<AppWindow>, settings: bool) {
    let _ = app.upgrade_in_event_loop(move |app| {
        if settings {
            app.set_page(4);
        }
        app.window().set_minimized(false);
        let _ = app.show();
        app.window().request_redraw();
        use slint::winit_030::WinitWindowAccessor;
        app.window()
            .with_winit_window(|window| window.focus_window());
    });
}

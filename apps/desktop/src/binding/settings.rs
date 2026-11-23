//! Own an editable settings draft independently of a running VPN operation.

use crate::AppWindow;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
use vmate_engine::{
    Port,
    model::{Appearance, Command, Preferences, ScanPreset, Snapshot},
};

pub(super) struct Session {
    draft: Rc<RefCell<Preferences>>,
    initialized: bool,
}

impl Session {
    pub(super) fn bind(app: &AppWindow, port: &Port) -> Self {
        let draft = Rc::new(RefCell::new(port.snapshot().preferences));
        let d = draft.clone();
        let weak = app.as_weak();
        app.on_select_preset(move |index| {
            if let Some(app) = weak.upgrade() {
                let mut p = d.borrow_mut();
                if let Err(error) = updated(&app, p.clone()) {
                    app.set_settings_feedback(format!("{error:#}").into());
                    return;
                }
                stage(&app, &mut p);
                app.set_editing_preset(index);
                if let Some(preset) = p.presets.get(index as usize) {
                    edit(&app, preset);
                }
            }
        });
        let d = draft.clone();
        let weak = app.as_weak();
        app.on_new_preset(move || {
            if let Some(app) = weak.upgrade() {
                let mut p = d.borrow_mut();
                if let Err(error) = updated(&app, p.clone()) {
                    app.set_settings_feedback(format!("{error:#}").into());
                    return;
                }
                stage(&app, &mut p);
                if p.presets.len() >= 16 {
                    return;
                }
                let name = (1..=17)
                    .map(|n| format!("Preset {n}"))
                    .find(|name| !p.presets.iter().any(|s| &s.name == name))
                    .unwrap();
                p.presets.push(ScanPreset {
                    name,
                    ..Default::default()
                });
                names(&app, &p);
                app.set_editing_preset(p.presets.len() as i32 - 1);
                edit(&app, p.presets.last().unwrap());
            }
        });
        let d = draft.clone();
        let p = port.clone();
        let weak = app.as_weak();
        app.on_save(move || {
            let Some(app) = weak.upgrade() else { return };
            match updated(&app, d.borrow().clone()).and_then(|v| p.send(Command::Save(v))) {
                Ok(()) => app.set_settings_feedback("Saving…".into()),
                Err(error) => app.set_settings_feedback(format!("{error:#}").into()),
            }
        });
        let d = draft.clone();
        let p = port.clone();
        let weak = app.as_weak();
        app.on_discard_settings(move || {
            if let Some(app) = weak.upgrade() {
                *d.borrow_mut() = p.snapshot().preferences;
                apply(&app, &d.borrow());
            }
        });
        Self {
            draft,
            initialized: false,
        }
    }

    pub(super) fn tick(&mut self, app: &AppWindow, snapshot: &Snapshot) {
        if !snapshot.ready {
            app.set_settings_saving(true);
            app.set_settings_feedback("Loading saved settings…".into());
            return;
        }
        if !self.initialized {
            *self.draft.borrow_mut() = snapshot.preferences.clone();
            apply(app, &snapshot.preferences);
            self.initialized = true;
        }
        let candidate = updated(app, self.draft.borrow().clone());
        let dirty = candidate.as_ref().map_or(true, |p| {
            format!("{p:?}") != format!("{:?}", snapshot.preferences)
        });
        app.set_settings_dirty(dirty);
        app.set_settings_saving(snapshot.saving);
        app.set_settings_valid(candidate.is_ok());
        app.set_settings_feedback(if snapshot.saving {
            "Saving…".into()
        } else if let Err(error) = candidate {
            format!("{error:#}").into()
        } else if dirty {
            "Unsaved changes".into()
        } else {
            "All changes saved".into()
        });
        stage(app, &mut self.draft.borrow_mut());
        names(app, &self.draft.borrow());
    }
}

fn names(app: &AppWindow, p: &Preferences) {
    let names: Vec<slint::SharedString> = p.presets.iter().map(|s| s.name.clone().into()).collect();
    if app.get_preset_names().iter().collect::<Vec<_>>() != names {
        app.set_preset_names(ModelRc::new(VecModel::from(names)));
    }
}

fn stage(app: &AppWindow, p: &mut Preferences) {
    if let Some(preset) = p.presets.get_mut(app.get_editing_preset() as usize) {
        preset.name = app.get_preset_name().trim().into();
        if let Ok(workers) = app.get_workers().parse() {
            preset.workers = workers;
        }
        if let Ok(timeout) = app.get_timeout().parse() {
            preset.timeout = timeout;
        }
    }
}

fn updated(app: &AppWindow, mut p: Preferences) -> anyhow::Result<Preferences> {
    stage(app, &mut p);
    let index = app.get_editing_preset() as usize;
    if let Some(preset) = p.presets.get_mut(index) {
        preset.workers = app
            .get_workers()
            .parse()
            .map_err(|_| anyhow::anyhow!("Workers must be 1–500"))?;
        preset.timeout = app
            .get_timeout()
            .parse()
            .map_err(|_| anyhow::anyhow!("Timeout must be 1–120 seconds"))?;
    }
    p.scan_preset = p
        .presets
        .get(app.get_scan_index() as usize)
        .ok_or_else(|| anyhow::anyhow!("Select a Scan preset"))?
        .name
        .clone();
    p.connect_preset = p
        .presets
        .get(app.get_connect_index() as usize)
        .ok_or_else(|| anyhow::anyhow!("Select a Connect preset"))?
        .name
        .clone();
    p.retry_limit = app
        .get_retry_limit()
        .parse()
        .map_err(|_| anyhow::anyhow!("Rescan limit must be 1–20"))?;
    p.auto_reconnect = app.get_auto_reconnect();
    p.reduced_motion = app.get_reduced();
    p.country_lookup = app.get_lookup();
    p.appearance = match app.get_appearance() {
        1 => Appearance::Light,
        2 => Appearance::Dark,
        _ => Appearance::System,
    };
    p.tray_enabled = app.get_tray_enabled();
    vmate_engine::validate_preferences(&p)?;
    Ok(p)
}

pub(super) fn apply(app: &AppWindow, p: &Preferences) {
    names(app, p);
    app.set_scan_index(
        p.presets
            .iter()
            .position(|s| s.name == p.scan_preset)
            .unwrap_or(0) as i32,
    );
    app.set_connect_index(
        p.presets
            .iter()
            .position(|s| s.name == p.connect_preset)
            .unwrap_or(0) as i32,
    );
    app.set_editing_preset(0);
    edit(app, &p.presets[0]);
    app.set_auto_reconnect(p.auto_reconnect);
    app.set_retry_limit(p.retry_limit.to_string().into());
    app.set_reduced(p.reduced_motion);
    app.set_lookup(p.country_lookup);
    app.set_appearance(match p.appearance {
        Appearance::System => 0,
        Appearance::Light => 1,
        Appearance::Dark => 2,
    });
    app.set_tray_enabled(p.tray_enabled);
}

fn edit(app: &AppWindow, preset: &ScanPreset) {
    app.set_preset_name(preset.name.clone().into());
    app.set_workers(preset.workers.to_string().into());
    app.set_timeout(preset.timeout.to_string().into());
}

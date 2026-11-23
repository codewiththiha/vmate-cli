//! Bind persisted presets without keeping a second engine preference store.

use crate::AppWindow;
use slint::{ComponentHandle, ModelRc, VecModel};
use vmate_engine::{
    Port,
    model::{Appearance, Command, Preferences, ScanPreset},
};

pub(super) fn bind(app: &AppWindow, port: &Port) {
    let p = port.clone();
    let weak = app.as_weak();
    app.on_select_preset(move |index| {
        if let Some(app) = weak.upgrade()
            && let Some(preset) = p.snapshot().preferences.presets.get(index as usize)
        {
            edit(&app, preset);
        }
    });
    let p = port.clone();
    let weak = app.as_weak();
    app.on_save(move || {
        let Some(app) = weak.upgrade() else { return };
        let result = updated(&app, p.snapshot().preferences)
            .and_then(|preferences| p.send(Command::Save(preferences)));
        if let Err(error) = result {
            app.set_diagnostic(format!("{error:#}").into());
        }
    });
}

fn updated(app: &AppWindow, mut p: Preferences) -> anyhow::Result<Preferences> {
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
    let preset = ScanPreset {
        name: app.get_preset_name().trim().into(),
        workers: app.get_workers().parse()?,
        timeout: app.get_timeout().parse()?,
    };
    if let Some(existing) = p.presets.iter_mut().find(|s| s.name == preset.name) {
        *existing = preset;
    } else {
        p.presets.push(preset);
    }
    p.retry_limit = app.get_retry_limit().parse()?;
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
    app.set_preset_names(ModelRc::new(VecModel::from(
        p.presets
            .iter()
            .map(|s| s.name.clone().into())
            .collect::<Vec<_>>(),
    )));
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
    let index = (app.get_editing_preset() as usize).min(p.presets.len() - 1);
    edit(app, &p.presets[index]);
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

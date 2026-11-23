//! Relay multi-selection and a previewed batch-export dialog.

use crate::{AppWindow, RelayRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use vmate_engine::{
    Port,
    model::{Command, ExportItem},
};

pub(super) fn bind(app: &AppWindow, port: &Port) {
    selection(app);
    let weak = app.as_weak();
    app.on_export_relay(move || {
        let Some(app) = weak.upgrade() else { return };
        let mut rows: Vec<_> = app.get_relays().iter().filter(|r| r.marked).collect();
        if rows.is_empty()
            && let Some(row) = app.get_relays().row_data(app.get_selected() as usize)
        {
            rows.push(row);
        }
        if rows.is_empty() {
            return;
        }
        app.set_export_rows(ModelRc::new(VecModel::from(rows)));
        preview(&app);
        app.set_export_open(true);
    });
    let weak = app.as_weak();
    app.on_preview_export(move || {
        if let Some(app) = weak.upgrade() {
            preview(&app);
        }
    });
    let weak = app.as_weak();
    app.on_choose_export_folder(move || {
        let weak = weak.clone();
        let _ = slint::spawn_local(async move {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await
                && let Some(app) = weak.upgrade()
            {
                app.set_export_directory(folder.path().to_string_lossy().as_ref().into());
            }
        });
    });
    let weak = app.as_weak();
    let p = port.clone();
    app.on_confirm_export(move || {
        if let Some(app) = weak.upgrade() {
            let items = app
                .get_export_rows()
                .iter()
                .map(|row| ExportItem {
                    path: row.path.to_string().into(),
                    filename: filename(&app, &row),
                })
                .collect();
            match p.send(Command::ExportBatch {
                items,
                directory: app.get_export_directory().to_string().into(),
            }) {
                Ok(()) => app.set_export_open(false),
                Err(error) => app.set_export_error(format!("{error:#}").into()),
            }
        }
    });
}

fn selection(app: &AppWindow) {
    let weak = app.as_weak();
    app.on_toggle_relay(move |index| {
        if let Some(app) = weak.upgrade() {
            let model = app.get_relays();
            if let Some(mut row) = model.row_data(index as usize) {
                row.marked = !row.marked;
                model.set_row_data(index as usize, row);
                app.set_selected(index);
                app.set_selection_mode(true);
                app.set_selected_count(model.iter().filter(|r| r.marked).count() as i32);
            }
        }
    });
    let weak = app.as_weak();
    app.on_select_all_relays(move |selected| {
        if let Some(app) = weak.upgrade() {
            let model = app.get_relays();
            for i in 0..model.row_count() {
                if let Some(mut row) = model.row_data(i) {
                    row.marked = selected;
                    model.set_row_data(i, row);
                }
            }
            app.set_selection_mode(selected);
            app.set_selected_count(if selected {
                model.row_count() as i32
            } else {
                0
            });
        }
    });
}

fn filename(app: &AppWindow, row: &RelayRow) -> String {
    let country = if app.get_export_full_country() {
        crate::country::name(&row.code)
            .unwrap_or("unknown")
            .to_lowercase()
            .replace(' ', "_")
    } else if row.code.is_empty() || row.code == "UNKNOWN" {
        "UNKNOWN".into()
    } else {
        row.code.to_string()
    };
    vmate_engine::export::filename(&row.name, &country, app.get_export_style())
}

fn preview(app: &AppWindow) {
    let rows = app.get_export_rows();
    let examples: Vec<_> = rows.iter().take(3).map(|r| filename(app, &r)).collect();
    app.set_export_preview(examples.join("\n").into());
    app.set_export_error("".into());
}

//! `vmate-cli recent`: show previously successful configs.

use crate::cli::RecentArgs;
use crate::settings::Settings;
use crate::ui::{clipboard, hyperlink, recent as recent_ui, term};
use anyhow::Result;
use vmate_core::db::ConfigRepo;
use vmate_core::db::models::StoredConfig;
use vmate_core::db::pool::init_pool;

pub async fn run(settings: &Settings, args: &RecentArgs) -> Result<()> {
    let pool = init_pool(&settings.db_path).await?;
    let repo = ConfigRepo::new(pool);

    let limit = if args.all {
        None
    } else {
        Some(args.limit as i64)
    };
    let entries = repo.list_recent(&settings.filter, limit, 0).await?;

    if entries.is_empty() {
        if settings.filter.is_empty() {
            println!("No successful configs found. Run `vmate-cli scan` first.");
        } else {
            println!("No successful configs matched filter: {}", settings.filter);
        }
        return Ok(());
    }

    if args.copy_first {
        let path = &entries[0].path;
        match clipboard::copy_to_clipboard(path) {
            Ok(method) => println!("Copied: {path} ({method})"),
            Err(err) => println!("copy failed: {err}"),
        }
        return Ok(());
    }

    if !args.no_tui && term::stdout_is_tty() {
        recent_ui::run(entries)?;
    } else {
        print_plain(&entries)?;
    }

    // Export previously-scanned configs matching the filter. Done after the
    // list so the result line is visible once the TUI (if any) exits.
    if let Some(export_dir) = &args.export {
        let dest = vmate_core::paths::expand_path(export_dir);
        let result = vmate_core::export::export_configs(&repo, &settings.filter, &dest).await?;
        println!(
            "Exported {} of {} configs to {}",
            result.exported,
            result.total,
            result.dest.display()
        );
    }

    Ok(())
}

fn print_plain(entries: &[StoredConfig]) -> Result<()> {
    let mut table = comfy_table::Table::new();
    table.set_header(["Country", "Path", "Last Success", "Successes"]);
    for entry in entries {
        let last = entry
            .last_success_at
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string());
        let path = if term::stdout_is_tty() {
            hyperlink::osc8_file_hyperlink(&entry.path).unwrap_or_else(|| entry.path.clone())
        } else {
            entry.path.clone()
        };
        table.add_row([
            entry.country.clone(),
            path,
            last,
            entry.success_count.to_string(),
        ]);
    }
    println!("{table}");
    Ok(())
}

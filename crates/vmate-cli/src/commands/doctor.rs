//! `vmate-cli doctor`: check the environment.

use crate::settings::Settings;
use anyhow::Result;
use comfy_table::Table;
use std::io::IsTerminal;
use vmate_core::db::ConfigRepo;
use vmate_core::db::models::ConfigStatus;
use vmate_core::db::pool::init_pool;
use vmate_core::system::is_root;

pub async fn run(settings: &Settings) -> Result<()> {
    let mut table = Table::new();
    table.set_header(["Check", "Status"]);

    table.add_row([
        format!("OpenVPN binary ({})", settings.openvpn_bin),
        status_text(binary_exists(&settings.openvpn_bin)),
    ]);
    table.add_row(["killall".to_string(), status_text(binary_exists("killall"))]);

    match init_pool(&settings.db_path).await {
        Ok(pool) => {
            table.add_row([
                format!("SQLite DB ({})", settings.db_path.display()),
                "ok".to_string(),
            ]);
            let repo = ConfigRepo::new(pool);
            match repo.journal_mode().await {
                Ok(mode) if mode.eq_ignore_ascii_case("wal") => {
                    table.add_row(["WAL mode".to_string(), "ok".to_string()]);
                }
                Ok(mode) => {
                    table.add_row(["WAL mode".to_string(), format!("unexpected ({mode})")]);
                }
                Err(e) => {
                    table.add_row(["WAL mode".to_string(), format!("error: {e}")]);
                }
            }

            let success = repo
                .count_configs(ConfigStatus::Success)
                .await
                .unwrap_or(-1);
            let failed = repo.count_configs(ConfigStatus::Failed).await.unwrap_or(-1);
            table.add_row(["DB success count".to_string(), success.to_string()]);
            table.add_row(["DB failed count".to_string(), failed.to_string()]);
        }
        Err(e) => {
            table.add_row([
                format!("SQLite DB ({})", settings.db_path.display()),
                format!("error: {e}"),
            ]);
        }
    }

    table.add_row([
        "Root".to_string(),
        if is_root() { "yes" } else { "no" }.to_string(),
    ]);
    table.add_row([
        "Terminal".to_string(),
        if std::io::stdout().is_terminal() {
            "interactive"
        } else {
            "piped"
        }
        .to_string(),
    ]);
    table.add_row([
        "Clipboard".to_string(),
        match arboard::Clipboard::new() {
            Ok(_) => "system".to_string(),
            Err(_) => "OSC 52 fallback".to_string(),
        },
    ]);
    table.add_row([
        "ipinfo token".to_string(),
        if settings.ipinfo_token.is_some() {
            "present".to_string()
        } else {
            "default (free key)".to_string()
        },
    ]);
    table.add_row([
        "killall -9 openvpn".to_string(),
        if settings.killall_enabled {
            "enabled (--killall)"
        } else {
            "disabled (per-process cleanup)"
        }
        .to_string(),
    ]);

    println!("{table}");
    Ok(())
}

fn status_text(ok: bool) -> String {
    if ok {
        "ok".to_string()
    } else {
        "missing".to_string()
    }
}

fn binary_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

//! `vmate-cli doctor`: check the environment.

use crate::settings::Settings;
use anyhow::Result;
use comfy_table::Table;
use std::io::IsTerminal;
use std::path::Path;
use vmate_core::db::ConfigRepo;
use vmate_core::db::models::ConfigStatus;
use vmate_core::db::pool::init_pool;

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
            match vmate_core::db::pool::journal_mode(&pool).await {
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

            let repo = ConfigRepo::new(pool);
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

    table.add_row(["Root".to_string(), vmate_core::system::root_summary()]);
    table.add_row([
        "Platform".to_string(),
        format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH),
    ]);
    table.add_row([
        "Home".to_string(),
        std::env::var("HOME").unwrap_or_else(|_| "-".to_string()),
    ]);
    match vmate_core::paths::config_dir() {
        Ok(dir) => table.add_row(["Config dir".to_string(), dir.display().to_string()]),
        Err(err) => table.add_row(["Config dir".to_string(), format!("error: {err}")]),
    };
    table.add_row(["DB access".to_string(), storage_status(&settings.db_path)]);
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

/// Whether the database — and the directory holding it — can be used by the
/// current user.
///
/// This is the check that makes the classic Linux failure visible: `sudo`
/// resets `HOME` there, so an older elevated run could create (and later leave
/// behind) a root-owned database that every normal run failed to open.
fn storage_status(db_path: &Path) -> String {
    let dir = match db_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    if !vmate_core::paths::writable_by_current_user(dir) {
        return format!("blocked ({} is not writable)", dir.display());
    }
    if !db_path.exists() {
        return "ok (not created yet)".to_string();
    }
    if vmate_core::paths::writable_by_current_user(db_path) {
        return "ok".to_string();
    }
    let owner = vmate_core::paths::owner_uid(db_path)
        .map(|uid| uid.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "read-only (owned by uid {owner}) — fix with: sudo chown -R \"$(id -u):$(id -g)\" {}",
        dir.display()
    )
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

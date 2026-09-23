//! SQLite connection pool with WAL mode and automatic migrations.

use anyhow::{Context, Result};
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

/// The type of the shared SQLite connection pool.
pub type DbPool = SqlitePool;

/// Open (or create) the database, enable WAL mode and run pending migrations.
pub async fn init_pool(db_path: &Path) -> Result<DbPool> {
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("cannot create database directory {}", parent.display())
            })?;
        }
    }

    // The database is opened by path, not through a `sqlite://` URL: URL
    // parsing treats everything after the first `?` as query parameters, so a
    // database path containing one would silently resolve somewhere else.
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("busy_timeout", "5000")
        .pragma("foreign_keys", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open SQLite database {}", db_path.display()))?;

    // Embed and run the migrations shipped with the crate.
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;

    // An elevated run creates root-owned database files. Handing them back
    // keeps one history shared between `sudo vmate-cli` and a normal run —
    // on Linux, where `sudo` resets HOME, that is exactly what used to break.
    repair_db_ownership(db_path);

    Ok(pool)
}

/// The current SQLite journal mode (used by `vmate-cli doctor`).
pub async fn journal_mode(pool: &DbPool) -> Result<String> {
    let row = sqlx::query("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
        .context("journal_mode check failed")?;
    Ok(row.get::<String, _>("journal_mode"))
}

/// Hand the database — and its WAL sidecars — back to the user who invoked
/// sudo. Best effort: a failed chown must never fail a scan or a connect.
fn repair_db_ownership(db_path: &Path) {
    crate::system::repair_ownership(db_path);
    if let Some(parent) = db_path.parent() {
        crate::system::repair_ownership(parent);
    }
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = db_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        crate::system::repair_ownership(Path::new(&sidecar));
    }
}

//! SQLite connection pool with WAL mode and automatic migrations.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

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

    // SQLite treats a single leading slash specially, so `sqlite:///abs/path`
    // is produced for absolute paths and `sqlite://rel/path` for relative ones.
    let url = format!("sqlite://{}", db_path.display());

    let options = SqliteConnectOptions::from_str(&url)
        .with_context(|| format!("invalid SQLite URL: {url}"))?
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("busy_timeout", "5000")
        .pragma("foreign_keys", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .context("failed to open SQLite database")?;

    // Embed and run the migrations shipped with the crate.
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;

    Ok(pool)
}

//! Success/failure/skip outcome recording.

use crate::country::CountryCode;
use crate::db::models::CountrySource;
use crate::db::repo::ConfigRepo;
use crate::hash;
use anyhow::{Context, Result};
use sqlx::Row;
use std::path::Path;

impl ConfigRepo {
    /// Record a successful connection, bumping `success_count`.
    pub async fn record_success(
        &self,
        path: &Path,
        sha256: &str,
        remote_host: Option<&str>,
        country: &CountryCode,
        country_source: CountrySource,
    ) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO configs (path, path_sha256, remote_host, country, country_source, \
                                  status, success_count, last_success_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'success', 1, \
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT(path) DO UPDATE SET \
                 path_sha256     = excluded.path_sha256, \
                 remote_host     = COALESCE(excluded.remote_host, configs.remote_host), \
                 country         = excluded.country, \
                 country_source  = excluded.country_source, \
                 status          = 'success', \
                 success_count   = configs.success_count + 1, \
                 last_success_at = excluded.last_success_at, \
                 updated_at      = excluded.updated_at \
             RETURNING id",
        )
        .bind(path.to_string_lossy().as_ref())
        .bind(sha256)
        .bind(remote_host)
        .bind(country.as_str())
        .bind(country_source.as_str())
        .fetch_one(&self.pool)
        .await
        .context("record_success failed")?;

        Ok(row.get::<i64, _>("id"))
    }

    /// Record a failed connection attempt for a config known only by path.
    pub async fn record_failure(&self, path: &Path, error: &str) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let path_sha = hash::sha256_str(&path_str);
        sqlx::query(
            "INSERT INTO configs (path, path_sha256, status, failure_count, last_failure_at, \
                                  last_error, updated_at) \
             VALUES (?1, ?2, 'failed', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, \
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT(path) DO UPDATE SET \
                 status          = 'failed', \
                 failure_count   = configs.failure_count + 1, \
                 last_failure_at = excluded.last_failure_at, \
                 last_error      = excluded.last_error, \
                 updated_at      = excluded.updated_at",
        )
        .bind(&path_str)
        .bind(&path_sha)
        .bind(error)
        .execute(&self.pool)
        .await
        .context("record_failure failed")?;
        Ok(())
    }

    /// Mark a config as manually skipped. The status is left untouched —
    /// skipping is not a failure and must not remove the config from history.
    pub async fn mark_skipped(&self, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE configs SET \
                 skipped_count   = skipped_count + 1, \
                 last_skipped_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
                 updated_at      = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .context("mark_skipped failed")?;
        Ok(())
    }
}

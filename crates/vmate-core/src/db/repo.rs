//! Repository over the SQLite database.

use crate::country::CountryCode;
use crate::db::models::{ConfigStatus, CountrySource, StoredConfig};
use crate::filter::CountryFilter;
use crate::hash;
use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;
use sqlx::{QueryBuilder, Row};
use std::path::Path;
use std::time::Duration;

/// All persistence for vmate lives behind this type.
///
/// Queries use `INSERT ... ON CONFLICT(path) DO UPDATE` so a config keeps one
/// row across scans, connections and failures.
#[derive(Clone)]
pub struct ConfigRepo {
    pool: SqlitePool,
}

impl ConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying pool (used by tests and diagnostics).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert the config if absent, otherwise update its metadata.
    /// Returns the row id.
    pub async fn upsert_config(
        &self,
        path: &Path,
        sha256: &str,
        remote_host: Option<&str>,
    ) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO configs (path, path_sha256, remote_host, status, updated_at) \
             VALUES (?1, ?2, ?3, 'unknown', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT(path) DO UPDATE SET \
                 path_sha256   = excluded.path_sha256, \
                 remote_host   = COALESCE(excluded.remote_host, configs.remote_host), \
                 updated_at    = excluded.updated_at \
             RETURNING id",
        )
        .bind(path.to_string_lossy().as_ref())
        .bind(sha256)
        .bind(remote_host)
        .fetch_one(&self.pool)
        .await
        .context("upsert_config failed")?;

        Ok(row.get::<i64, _>("id"))
    }

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

    /// Record a failed connection for a config already present (by id).
    pub async fn mark_failed(&self, id: i64, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE configs SET \
                 status          = 'failed', \
                 failure_count   = failure_count + 1, \
                 last_failure_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
                 last_error      = ?2, \
                 updated_at      = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?1",
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .context("mark_failed failed")?;
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

    /// Fetch a config by its absolute path.
    pub async fn config_by_path(&self, path: &Path) -> Result<Option<StoredConfig>> {
        let row = sqlx::query_as::<_, StoredConfig>("SELECT * FROM configs WHERE path = ?1")
            .bind(path.to_string_lossy().as_ref())
            .fetch_optional(&self.pool)
            .await
            .context("config_by_path failed")?;
        Ok(row)
    }

    /// List successful configs, newest first, honoring the country filter.
    pub async fn list_recent(
        &self,
        filter: &CountryFilter,
        limit: Option<i64>,
        offset: i64,
    ) -> Result<Vec<StoredConfig>> {
        let mut builder = QueryBuilder::new("SELECT * FROM configs WHERE status = 'success'");

        if !filter.is_empty() {
            builder.push(" AND country IN (");
            let mut separated = builder.separated(", ");
            for code in filter.countries() {
                separated.push_bind(code.as_str());
            }
            separated.push_unseparated(")");
        }

        builder.push(" ORDER BY last_success_at DESC, success_count DESC");

        // SQLite requires LIMIT before OFFSET, so only emit the pair together.
        if let Some(limit) = limit {
            builder
                .push(" LIMIT ")
                .push_bind(limit)
                .push(" OFFSET ")
                .push_bind(offset);
        }

        let rows = builder
            .build_query_as::<StoredConfig>()
            .fetch_all(&self.pool)
            .await
            .context("list_recent failed")?;
        Ok(rows)
    }

    /// Candidates for the connect retry queue.
    ///
    /// Includes configs that succeeded at least once, respects the filter,
    /// avoids configs that failed within `cooldown`, prefers configs that have
    /// been skipped the least, and randomizes ties.
    pub async fn connect_candidates(
        &self,
        filter: &CountryFilter,
        cooldown: Duration,
    ) -> Result<Vec<StoredConfig>> {
        let cooldown_secs = cooldown.as_secs();

        let mut builder = QueryBuilder::new(
            "SELECT * FROM configs WHERE status = 'success' AND (last_failure_at IS NULL OR \
             last_failure_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ",
        );
        builder.push_bind(format!("-{cooldown_secs} seconds"));
        builder.push("))");

        if !filter.is_empty() {
            builder.push(" AND country IN (");
            let mut separated = builder.separated(", ");
            for code in filter.countries() {
                separated.push_bind(code.as_str());
            }
            separated.push_unseparated(")");
        }

        builder
            .push(" ORDER BY skipped_count ASC, last_skipped_at ASC, failure_count ASC, RANDOM()");

        let rows = builder
            .build_query_as::<StoredConfig>()
            .fetch_all(&self.pool)
            .await
            .context("connect_candidates failed")?;
        Ok(rows)
    }

    /// All successful configs (used by export).
    pub async fn all_successful(&self) -> Result<Vec<StoredConfig>> {
        let rows = sqlx::query_as::<_, StoredConfig>(
            "SELECT * FROM configs WHERE status = 'success' ORDER BY last_success_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .context("all_successful failed")?;
        Ok(rows)
    }

    /// Look up a cached IP -> country mapping.
    pub async fn get_cached_country_by_ip(&self, ip: &str) -> Result<Option<CountryCode>> {
        let row = sqlx::query("SELECT country FROM ip_country_cache WHERE ip = ?1")
            .bind(ip)
            .fetch_optional(&self.pool)
            .await
            .context("get_cached_country_by_ip failed")?;

        let Some(row) = row else { return Ok(None) };
        let country: String = row.get("country");
        let code = CountryCode::new(country).ok();
        // A cached UNKNOWN is not a useful hit: it means a lookup previously
        // failed, so treat it as a miss and let the API be retried.
        if code.as_ref().is_some_and(|c| c.is_unknown()) {
            Ok(None)
        } else {
            Ok(code)
        }
    }

    /// Persist an IP -> country mapping.
    pub async fn cache_country_for_ip(&self, ip: &str, country: &CountryCode) -> Result<()> {
        sqlx::query(
            "INSERT INTO ip_country_cache (ip, country, fetched_at) \
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
             ON CONFLICT(ip) DO UPDATE SET \
                 country    = excluded.country, \
                 fetched_at = excluded.fetched_at",
        )
        .bind(ip)
        .bind(country.as_str())
        .execute(&self.pool)
        .await
        .context("cache_country_for_ip failed")?;
        Ok(())
    }

    /// Current journal mode (used by `vmate doctor`).
    pub async fn journal_mode(&self) -> Result<String> {
        let row = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&self.pool)
            .await
            .context("journal_mode check failed")?;
        Ok(row.get::<String, _>("journal_mode"))
    }

    /// Count configs with a given status (used by `vmate doctor`).
    pub async fn count_configs(&self, status: ConfigStatus) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM configs WHERE status = ?1")
            .bind(status.as_str())
            .fetch_one(&self.pool)
            .await
            .context("count_configs failed")?;
        Ok(row.get::<i64, _>("n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::init_pool;

    /// Create a repo plus a tempdir that must stay alive for the whole test —
    /// dropping it deletes the database file underneath the pool.
    async fn test_repo() -> (ConfigRepo, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let repo = ConfigRepo::new(init_pool(&path).await.expect("init pool"));
        (repo, dir)
    }

    #[tokio::test]
    async fn upsert_is_idempotent() {
        let (repo, _dir) = test_repo().await;
        let path = std::path::Path::new("/tmp/fake-jp.ovpn");
        let a = repo
            .upsert_config(path, "abc", Some("a.example.com"))
            .await
            .unwrap();
        let b = repo
            .upsert_config(path, "def", Some("b.example.com"))
            .await
            .unwrap();
        assert_eq!(a, b);
        let stored = repo.config_by_path(path).await.unwrap().unwrap();
        assert_eq!(stored.remote_host.as_deref(), Some("b.example.com"));
    }

    #[tokio::test]
    async fn record_success_then_recent_returns_it() {
        let (repo, _dir) = test_repo().await;
        let path = std::path::Path::new("/tmp/success.ovpn");
        let country = CountryCode::new("jp").unwrap();
        repo.record_success(
            path,
            "abc",
            Some("jp.example.com"),
            &country,
            CountrySource::FileName,
        )
        .await
        .unwrap();

        let filter = CountryFilter::new();
        let recent = repo.list_recent(&filter, None, 0).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].country, "JP");
        assert_eq!(recent[0].status, ConfigStatus::Success);
    }

    #[tokio::test]
    async fn filter_excludes_non_matching_countries() {
        let (repo, _dir) = test_repo().await;
        let jp = std::path::Path::new("/tmp/jp.ovpn");
        let us = std::path::Path::new("/tmp/us.ovpn");
        repo.record_success(
            jp,
            "a",
            None,
            &CountryCode::new("jp").unwrap(),
            CountrySource::FileName,
        )
        .await
        .unwrap();
        repo.record_success(
            us,
            "b",
            None,
            &CountryCode::new("us").unwrap(),
            CountrySource::FileName,
        )
        .await
        .unwrap();

        let filter = CountryFilter::from_args(&["jp".to_string()]).unwrap();
        let recent = repo.list_recent(&filter, None, 0).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].path, "/tmp/jp.ovpn");
    }

    #[tokio::test]
    async fn skip_does_not_remove_from_history() {
        let (repo, _dir) = test_repo().await;
        let path = std::path::Path::new("/tmp/skip-me.ovpn");
        let id = repo
            .record_success(
                path,
                "abc",
                None,
                &CountryCode::new("kr").unwrap(),
                CountrySource::FileName,
            )
            .await
            .unwrap();
        repo.mark_skipped(id).await.unwrap();

        let recent = repo
            .list_recent(&CountryFilter::new(), None, 0)
            .await
            .unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].skipped_count, 1);
        assert_eq!(recent[0].status, ConfigStatus::Success);
    }

    #[tokio::test]
    async fn geo_cache_round_trips() {
        let (repo, _dir) = test_repo().await;
        assert!(
            repo.get_cached_country_by_ip("1.2.3.4")
                .await
                .unwrap()
                .is_none()
        );
        let country = CountryCode::new("de").unwrap();
        repo.cache_country_for_ip("1.2.3.4", &country)
            .await
            .unwrap();
        assert_eq!(
            repo.get_cached_country_by_ip("1.2.3.4").await.unwrap(),
            Some(country)
        );
    }

    #[tokio::test]
    async fn journal_mode_is_wal() {
        let (repo, _dir) = test_repo().await;
        assert_eq!(repo.journal_mode().await.unwrap(), "wal");
    }
}

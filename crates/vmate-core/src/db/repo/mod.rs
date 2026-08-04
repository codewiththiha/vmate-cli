//! The config repository: struct + core CRUD, partitioned by concern into
//! sibling modules (outcomes, queries, geo_cache).

mod geo_cache;
mod outcomes;
mod queries;

use crate::db::models::StoredConfig;
use anyhow::{Context, Result};
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use std::path::Path;

/// All persistence for vmate-cli lives behind this type.
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

    /// Fetch a config by its absolute path.
    pub async fn config_by_path(&self, path: &Path) -> Result<Option<StoredConfig>> {
        let row = sqlx::query_as::<_, StoredConfig>("SELECT * FROM configs WHERE path = ?1")
            .bind(path.to_string_lossy().as_ref())
            .fetch_optional(&self.pool)
            .await
            .context("config_by_path failed")?;
        Ok(row)
    }

    /// Remove a config from history entirely (Go parity: drop after repeated
    /// failed connection attempts).
    pub async fn delete_config_by_path(&self, path: &Path) -> Result<()> {
        sqlx::query("DELETE FROM configs WHERE path = ?1")
            .bind(path.to_string_lossy().as_ref())
            .execute(&self.pool)
            .await
            .context("delete_config_by_path failed")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::country::CountryCode;
    use crate::db::models::{ConfigStatus, CountrySource};
    use crate::db::pool::init_pool;
    use crate::filter::CountryFilter;

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
    async fn delete_removes_from_history() {
        let (repo, _dir) = test_repo().await;
        let path = std::path::Path::new("/tmp/drop.ovpn");
        repo.record_success(
            path,
            "x",
            None,
            &CountryCode::new("jp").unwrap(),
            CountrySource::FileName,
        )
        .await
        .unwrap();
        assert_eq!(
            repo.list_recent(&CountryFilter::new(), None, 0)
                .await
                .unwrap()
                .len(),
            1
        );
        repo.delete_config_by_path(path).await.unwrap();
        assert!(repo.config_by_path(path).await.unwrap().is_none());
        assert!(
            repo.list_recent(&CountryFilter::new(), None, 0)
                .await
                .unwrap()
                .is_empty()
        );
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
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = init_pool(&dir.path().join("test.db"))
            .await
            .expect("init pool");
        assert_eq!(crate::db::pool::journal_mode(&pool).await.unwrap(), "wal");
    }
}

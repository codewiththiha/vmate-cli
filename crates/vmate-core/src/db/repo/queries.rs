//! Read queries over the configs table.

use crate::db::models::{ConfigStatus, StoredConfig};
use crate::db::repo::ConfigRepo;
use crate::filter::CountryFilter;
use anyhow::{Context, Result};
use sqlx::{QueryBuilder, Row};
use std::time::Duration;

impl ConfigRepo {
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

    /// Count configs with a given status (used by `vmate-cli doctor`).
    pub async fn count_configs(&self, status: ConfigStatus) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS n FROM configs WHERE status = ?1")
            .bind(status.as_str())
            .fetch_one(&self.pool)
            .await
            .context("count_configs failed")?;
        Ok(row.get::<i64, _>("n"))
    }
}

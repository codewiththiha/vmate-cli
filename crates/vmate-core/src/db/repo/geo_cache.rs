//! IP -> country cache.

use crate::country::CountryCode;
use crate::db::repo::ConfigRepo;
use anyhow::{Context, Result};
use sqlx::Row;

impl ConfigRepo {
    /// Update country metadata without recording a successful connection.
    pub async fn update_country(
        &self,
        path: &std::path::Path,
        lookup: &crate::geo::CountryLookup,
    ) -> Result<()> {
        sqlx::query("UPDATE configs SET country = ?1, country_source = ?2 WHERE path = ?3")
            .bind(lookup.country.as_str())
            .bind(lookup.source.as_str())
            .bind(path.to_string_lossy().as_ref())
            .execute(&self.pool)
            .await
            .context("update_country failed")?;
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lookup_does_not_change_connection_history() {
        let dir = tempfile::tempdir().unwrap();
        let repo = ConfigRepo::new(
            crate::db::init_pool(&dir.path().join("test.db"))
                .await
                .unwrap(),
        );
        let path = std::path::Path::new("/tmp/relay.ovpn");
        repo.record_success(
            path,
            "hash",
            None,
            &CountryCode::unknown(),
            crate::db::CountrySource::Unknown,
        )
        .await
        .unwrap();
        let before = repo.config_by_path(path).await.unwrap().unwrap();
        repo.update_country(
            path,
            &crate::geo::CountryLookup {
                country: CountryCode::new("JP").unwrap(),
                source: crate::db::CountrySource::IpApi,
            },
        )
        .await
        .unwrap();
        let after = repo.config_by_path(path).await.unwrap().unwrap();
        assert_eq!(after.country, "JP");
        assert_eq!(after.country_source, "ip_api");
        assert_eq!(after.success_count, before.success_count);
        assert_eq!(after.last_success_at, before.last_success_at);
        assert_eq!(after.status, before.status);
    }
}

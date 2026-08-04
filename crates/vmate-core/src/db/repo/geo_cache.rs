//! IP -> country cache.

use crate::country::CountryCode;
use crate::db::repo::ConfigRepo;
use anyhow::{Context, Result};
use sqlx::Row;

impl ConfigRepo {
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

//! Country detection for configs: filename heuristics, DNS resolution and
//! ipinfo.io lookups, with the SQLite table doubling as the cache.

pub mod cache;
pub mod ipinfo;

use crate::country::CountryCode;
use crate::db::ConfigRepo;
use crate::db::models::CountrySource;
use crate::ovpn::parser::parse_remote_host;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

/// A country together with how it was determined.
#[derive(Debug, Clone)]
pub struct CountryLookup {
    pub country: CountryCode,
    pub source: CountrySource,
}

/// Abstraction over resolving a config to a country.
#[async_trait]
pub trait GeoLocator: Send + Sync {
    async fn country_for_config(&self, config: &Path) -> Result<CountryLookup>;
}

/// Production locator.
///
/// Resolution order:
/// 1. A two-letter code embedded in the file name (fast, no network).
/// 2. The SQLite IP cache.
/// 3. ipinfo.io — using the free token from the original Go tool by default,
///    overridable with `--ipinfo-token` / `IPINFO_TOKEN`.
///
/// Failures degrade to `UNKNOWN` — geo lookup must never abort a scan.
pub struct IpInfoGeoLocator {
    pub client: reqwest::Client,
    pub repo: Arc<ConfigRepo>,
    pub token: Option<String>,
    pub memo: cache::GeoMemo,
}

#[async_trait]
impl GeoLocator for IpInfoGeoLocator {
    async fn country_for_config(&self, config: &Path) -> Result<CountryLookup> {
        // Fast path: a country code embedded in the file name.
        if let Some(code) = country_from_filename(config) {
            return Ok(CountryLookup {
                country: code,
                source: CountrySource::FileName,
            });
        }

        let host = match parse_remote_host(config) {
            Ok(h) => h,
            Err(_) => {
                return Ok(CountryLookup {
                    country: CountryCode::unknown(),
                    source: CountrySource::Unknown,
                });
            }
        };

        let Some(ip) = resolve_ip(&host).await else {
            return Ok(CountryLookup {
                country: CountryCode::unknown(),
                source: CountrySource::Unknown,
            });
        };

        if let Some(country) = self.memo.get(&ip) {
            return Ok(CountryLookup {
                country,
                source: CountrySource::RemoteHost,
            });
        }

        if let Some(country) = self.repo.get_cached_country_by_ip(&ip).await? {
            self.memo.set(ip, country.clone());
            return Ok(CountryLookup {
                country,
                source: CountrySource::RemoteHost,
            });
        }

        let country = match ipinfo::lookup_country(&self.client, &ip, self.token.as_deref()).await {
            Some(c) => c,
            None => CountryCode::unknown(),
        };
        let source = if country.is_unknown() {
            CountrySource::Unknown
        } else {
            CountrySource::IpApi
        };

        // Only persist real countries. Caching UNKNOWN would make a transient
        // API failure permanent for that IP.
        if !country.is_unknown() {
            let _ = self.repo.cache_country_for_ip(&ip, &country).await;
        }
        self.memo.set(ip, country.clone());

        Ok(CountryLookup { country, source })
    }
}

impl IpInfoGeoLocator {
    /// Build a locator with a per-session in-memory memo layered over SQLite.
    ///
    /// When no token is supplied the free token from the original Go tool is
    /// used, so country lookup works without any configuration.
    pub fn new(repo: Arc<ConfigRepo>, token: Option<String>) -> Self {
        let token = token.or_else(|| Some(ipinfo::DEFAULT_IPINFO_TOKEN.to_string()));
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("reqwest client build cannot fail"),
            repo,
            token,
            memo: cache::GeoMemo::new(),
        }
    }
}

/// Resolve a hostname to its first IPv4/IPv6 address string.
pub async fn resolve_ip(host: &str) -> Option<String> {
    match tokio::net::lookup_host((host, 0)).await {
        Ok(mut addrs) => addrs.next().map(|a| a.ip().to_string()),
        Err(e) => {
            tracing::debug!(%host, error = %e, "DNS resolution failed");
            None
        }
    }
}

/// Extract a two-letter country code embedded in a file name.
///
/// VPN Gate names look like `vpngate_20260801_jp_...ovpn`; the heuristic
/// scans for underscore/hyphen-delimited tokens of exactly two ASCII letters.
pub fn country_from_filename(config: &Path) -> Option<CountryCode> {
    let name = config.file_name()?.to_string_lossy().to_lowercase();
    for part in name.split(['_', '-', '.']) {
        if part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            if let Ok(code) = CountryCode::new(part) {
                return Some(code);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_heuristic_finds_country() {
        let p = Path::new("/configs/vpngate_20260801_jp_vpn-gate.ovpn");
        assert_eq!(
            country_from_filename(p).map(|c| c.as_str().to_string()),
            Some("JP".to_string())
        );
    }

    #[test]
    fn filename_heuristic_handles_upper() {
        let p = Path::new("/configs/KR-seoul.ovpn");
        assert_eq!(
            country_from_filename(p).map(|c| c.as_str().to_string()),
            Some("KR".to_string())
        );
    }

    #[test]
    fn filename_heuristic_returns_none_when_absent() {
        let p = Path::new("/configs/vpn-gate.ovpn");
        assert!(country_from_filename(p).is_none());
    }
}

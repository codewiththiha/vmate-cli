//! ipinfo.io lookup.

use crate::country::CountryCode;
use reqwest::Client;

/// Response shape for `https://api.ipinfo.io/lite/{ip}`. The free endpoint
/// returns `country_code`; the standard endpoint returns `country`.
#[derive(serde::Deserialize)]
pub struct IpInfoResponse {
    #[serde(alias = "country_code", alias = "country")]
    pub country: Option<String>,
}

/// Look up the country for an IP address.
///
/// Returns `None` on any failure (HTTP error, timeout, parse error) — callers
/// degrade to `UNKNOWN` and keep going. Requires a token; without one this
/// returns `None` immediately rather than hitting a 403.
pub async fn lookup_country(client: &Client, ip: &str, token: Option<&str>) -> Option<CountryCode> {
    let token = token?;

    let url = format!("https://api.ipinfo.io/lite/{ip}?token={token}");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!(%ip, status = %resp.status(), "ipinfo lookup failed");
        return None;
    }

    let body: IpInfoResponse = resp.json().await.ok()?;
    let country = body.country?;
    CountryCode::new(country).ok()
}

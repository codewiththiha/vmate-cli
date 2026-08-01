//! ipinfo.io lookup.

use crate::country::CountryCode;
use reqwest::Client;

/// Free ipinfo.io token, taken verbatim from the original Go vmate-cli so
/// country lookup works out of the box. Override with `--ipinfo-token` or
/// `IPINFO_TOKEN`.
pub const DEFAULT_IPINFO_TOKEN: &str = "44936a1f60206d";

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
    let resp = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::debug!(%ip, error = %e, "ipinfo request failed");
            return None;
        }
    };
    if !resp.status().is_success() {
        tracing::debug!(%ip, status = %resp.status(), "ipinfo lookup failed");
        return None;
    }

    let body: IpInfoResponse = match resp.json().await {
        Ok(body) => body,
        Err(e) => {
            tracing::debug!(%ip, error = %e, "ipinfo response parse failed");
            return None;
        }
    };
    let country = body.country?;
    CountryCode::new(country).ok()
}

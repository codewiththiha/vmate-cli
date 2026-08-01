//! ipinfo.io lookup.
//!
//! The request mirrors the original Go vmate-cli exactly:
//! `GET https://api.ipinfo.io/lite/{ip}?token={token}` decoding only the
//! `country_code` field from the response. The lite endpoint also returns a
//! full `country` name, so the struct must NOT also map `country` — doing so
//! makes serde reject the payload as a duplicate field.

use crate::country::CountryCode;
use reqwest::Client;

/// Free ipinfo.io token, taken verbatim from the original Go vmate-cli so
/// country lookup works out of the box. Override with `--ipinfo-token` or
/// `IPINFO_TOKEN`.
pub const DEFAULT_IPINFO_TOKEN: &str = "44936a1f60206d";

/// Response shape for `https://api.ipinfo.io/lite/{ip}`.
///
/// Only `country_code` is decoded — the same field the Go version read. The
/// lite payload additionally carries `country` (the full name) and other
/// fields, which are intentionally ignored.
#[derive(serde::Deserialize)]
pub struct IpInfoResponse {
    pub country_code: Option<String>,
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
    let country = body.country_code?;
    CountryCode::new(country).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the exact lite-endpoint payload, which contains
    /// both `country_code` and `country`. Decoding only `country_code` must
    /// not be rejected as a duplicate field.
    #[test]
    fn decodes_country_code_from_lite_payload() {
        let body: IpInfoResponse = serde_json::from_str(
            r#"{
                "ip": "1.1.1.1",
                "asn": "AS13335",
                "as_name": "Cloudflare, Inc.",
                "as_domain": "cloudflare.com",
                "country_code": "AU",
                "country": "Australia",
                "continent_code": "OC",
                "continent": "Oceania"
            }"#,
        )
        .expect("lite payload should decode");
        assert_eq!(body.country_code.as_deref(), Some("AU"));
    }
}

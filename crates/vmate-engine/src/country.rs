//! Explicit bounded country refresh without scanning or connecting.

use crate::Port;
use anyhow::{Context, Result, ensure};
use std::{path::Path, sync::Arc, time::Duration};
use vmate_core::{
    db::ConfigRepo,
    geo::{GeoLocator, IpInfoGeoLocator},
};

pub(super) async fn lookup(port: &Port, path: &Path) -> Result<()> {
    let repo = Arc::new(ConfigRepo::new(
        vmate_core::db::init_pool(&port.configuration.database).await?,
    ));
    ensure!(
        repo.config_by_path(path).await?.is_some(),
        "Relay is not in history"
    );
    let locator = IpInfoGeoLocator::new(repo.clone(), std::env::var("IPINFO_TOKEN").ok());
    let cancel = port.state.lock().unwrap().operation.clone();
    port.state.lock().unwrap().snapshot.message = "Looking up relay country…".into();
    let lookup = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Ok(()),
        result = tokio::time::timeout(Duration::from_secs(10), locator.country_for_config(path)) => {
            result.context("Country lookup timed out")??
        }
    };
    if !lookup.country.is_unknown() {
        repo.update_country(path, &lookup).await?;
    }
    let relays = crate::platform::recent_from(&repo).await?;
    let mut state = port.state.lock().unwrap();
    state.snapshot.relays = relays;
    state.snapshot.message = if lookup.country.is_unknown() {
        "Country unavailable. You can try again later.".into()
    } else {
        "Relay country updated. This is not verified VPN exit location.".into()
    };
    Ok(())
}

//! Compose existing core scanning without adding frontend dependencies.

use crate::{Port, model::Relay};
use anyhow::Result;
use async_trait::async_trait;
use std::{path::Path, sync::Arc, time::Duration};
use vmate_core::geo::{CountryLookup, GeoLocator, IpInfoGeoLocator};
use vmate_core::scan::{ScanOptions, ScanProgress, ScanService};
use vmate_core::system::{ProcessRegistry, RealProcessKiller};
use vmate_core::{builtin, country::CountryCode, db::ConfigRepo, filter::CountryFilter};

pub(super) async fn run(port: &Port) -> Result<()> {
    let preferences = port.snapshot().preferences;
    crate::validate_preferences(&preferences)?;
    let dir = port.configuration.config_dir.join("builtin/vpn-gate/udp");
    for config in builtin::enumerate(builtin::Provider::VpnGate, builtin::Proto::Udp) {
        builtin::materialize(&config, &dir)?;
    }
    let pool = vmate_core::db::init_pool(&port.configuration.database).await?;
    let repo = Arc::new(ConfigRepo::new(pool));
    let geo: Arc<dyn GeoLocator> = if preferences.country_lookup {
        Arc::new(IpInfoGeoLocator::new(repo.clone(), None))
    } else {
        Arc::new(PrivateGeo)
    };
    let service = ScanService {
        tester: Arc::new(vmate_core::ovpn::process::RealVpnTester {
            bin: port.configuration.openvpn.to_string_lossy().into(),
            killer: Arc::new(RealProcessKiller {
                killall_enabled: false,
            }),
            registry: Arc::new(ProcessRegistry::new()),
        }),
        geo,
        repo,
    };
    let options = ScanOptions {
        dir,
        limit: usize::MAX,
        timeout: Duration::from_secs(preferences.timeout),
        workers: preferences.workers,
        modify: false,
        backup: false,
        no_save: false,
        filter: CountryFilter::new(),
    };
    let cancel = {
        let mut state = port.state.lock().unwrap();
        state.snapshot.relays.clear();
        state.snapshot.completed = 0;
        state.snapshot.status = "Scanning".into();
        state.snapshot.message = "TLS handshakes only. Your system VPN is not changed.".into();
        state.operation.clone()
    };
    let report = service
        .scan(&options, Arc::new(Progress(port.clone())), cancel.clone())
        .await?;
    let mut state = port.state.lock().unwrap();
    let message = format!(
        "{}: {} tested, {} handshakes succeeded. No system tunnel created.",
        if cancel.is_cancelled() {
            "Scan stopped"
        } else {
            "Scan complete"
        },
        report.tested,
        report.success
    );
    state.snapshot.message = message.clone();
    state.event(message);
    Ok(())
}

struct PrivateGeo;
#[async_trait]
impl GeoLocator for PrivateGeo {
    async fn country_for_config(&self, _config: &Path) -> Result<CountryLookup> {
        Ok(CountryLookup {
            country: CountryCode::unknown(),
            source: vmate_core::db::CountrySource::Unknown,
        })
    }
}

struct Progress(Port);
impl ScanProgress for Progress {
    fn total(&self, total: usize) {
        self.0.state.lock().unwrap().snapshot.total = total;
    }
    fn tested(&self) {
        self.0.state.lock().unwrap().snapshot.completed += 1;
    }
    fn ok(&self) {}
    fn matched(&self) {}
    fn failed(&self, _path: &Path) {}
    fn success(&self, path: &Path, country: &CountryCode) {
        let mut state = self.0.state.lock().unwrap();
        if state.snapshot.relays.len() < 500 {
            state.snapshot.relays.push(Relay {
                path: path.into(),
                country: country.to_string(),
                name: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                checked: "Just now · handshake passed".into(),
            });
        }
    }
}

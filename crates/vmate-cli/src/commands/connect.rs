//! `vmate connect`: connect to a config with intelligent retry and skip.

use crate::cli::ConnectArgs;
use crate::settings::Settings;
use anyhow::{Result, bail};
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use vmate_core::connect::{Candidate, ConnectOptions, ConnectQueue, ConnectService};
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::geo::{GeoLocator, IpInfoGeoLocator};
use vmate_core::ovpn::process::RealOpenVpnRunner;
use vmate_core::paths;
use vmate_core::system::{ProcessKiller, RealProcessKiller, require_root_for};

pub async fn run(settings: &Settings, args: &ConnectArgs, verbose: &Verbosity) -> Result<()> {
    require_root_for("run OpenVPN connections")?;

    let pool = init_pool(&settings.db_path).await?;
    let repo = Arc::new(ConfigRepo::new(pool));

    let queue = build_queue(settings, args, repo.clone()).await?;

    if queue.is_empty() {
        if settings.filter.is_empty() {
            println!("No connectable configs found in history.");
        } else {
            println!("No connectable configs matched filter: {}", settings.filter);
        }
        return Ok(());
    }

    let killer: Arc<dyn ProcessKiller> = Arc::new(RealProcessKiller {
        killall_enabled: settings.killall_enabled,
    });
    let runner = Arc::new(RealOpenVpnRunner {
        bin: settings.openvpn_bin.clone(),
    });

    let options = ConnectOptions {
        connect_timeout: args.connect_timeout,
        verbose: crate::app::is_verbose(verbose),
        killall_enabled: settings.killall_enabled,
    };

    let mut host =
        crate::ui::connect::ConnectTui::new(args.no_interactive, settings.filter.to_display())?;
    let service = ConnectService {
        runner,
        killer,
        repo,
        options,
    };
    service.run(queue, &mut host).await?;
    Ok(())
}

async fn build_queue(
    settings: &Settings,
    args: &ConnectArgs,
    repo: Arc<ConfigRepo>,
) -> Result<ConnectQueue> {
    let mut candidates: Vec<Candidate> = Vec::new();

    if let Some(explicit) = &args.config {
        let path = paths::expand_path(explicit);
        let geo = IpInfoGeoLocator::new(repo.clone(), settings.ipinfo_token.clone());
        let lookup = geo.country_for_config(&path).await?;
        let country = lookup.country;

        if country.is_unknown() {
            println!(
                "warning: could not determine country for {}",
                path.display()
            );
        }
        if args.strict_filter && !settings.filter.matches(country.as_str()) {
            bail!(
                "config {} (country {}) does not match filter {}",
                path.display(),
                country,
                settings.filter
            );
        }

        let id = repo.config_by_path(&path).await?.map(|c| c.id).unwrap_or(0);
        candidates.push(Candidate {
            id,
            path: path.to_string_lossy().to_string(),
            country: country.to_string(),
        });
    }

    // Stored candidates (respecting the filter) act as fallbacks.
    let stored = repo
        .connect_candidates(&settings.filter, args.cooldown)
        .await?;
    for stored_config in stored {
        let candidate = Candidate::from(&stored_config);
        if !candidates.iter().any(|c| c.path == candidate.path) {
            candidates.push(candidate);
        }
    }

    Ok(ConnectQueue::new(candidates))
}

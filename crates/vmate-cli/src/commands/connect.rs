//! `vmate-cli connect`: connect to a config with intelligent retry and skip.

use crate::cli::ConnectArgs;
use crate::settings::Settings;
use anyhow::{Result, bail};
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use std::time::Duration;
use vmate_core::connect::{Candidate, ConnectOptions, ConnectQueue, ConnectService};
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::geo::{GeoLocator, IpInfoGeoLocator};
use vmate_core::ovpn::process::RealOpenVpnRunner;
use vmate_core::paths;
use vmate_core::settings::UserSettings;
use vmate_core::system::{ProcessKiller, RealProcessKiller, require_root_for};

/// Effective connect tunables after resolving explicit flags, persisted
/// settings and built-in defaults.
pub(crate) struct ResolvedConnect {
    pub connect_timeout: Duration,
    pub cooldown: Duration,
    pub retry_count: u32,
    pub stability_grace: Duration,
}

/// Resolve connect tunables as `explicit CLI flag → persisted → built-in`.
pub(crate) fn resolve_connect(us: &UserSettings, args: &ConnectArgs) -> ResolvedConnect {
    ResolvedConnect {
        connect_timeout: us.connect_timeout(args.connect_timeout),
        cooldown: us.cooldown(args.cooldown),
        retry_count: us.retry_count(args.retry_count),
        stability_grace: us.stability_grace(args.stability_grace),
    }
}

/// Persist the explicitly-passed connect defaults, then confirm where they
/// were written.
pub(crate) fn persist_connect_defaults(us: &mut UserSettings, args: &ConnectArgs) -> Result<()> {
    if let Some(v) = args.connect_timeout {
        us.connect_timeout_secs = Some(v.as_secs());
    }
    if let Some(v) = args.cooldown {
        us.cooldown_secs = Some(v.as_secs());
    }
    if let Some(v) = args.retry_count {
        us.retry_count = Some(v);
    }
    if let Some(v) = args.stability_grace {
        us.stability_grace_secs = Some(v.as_secs());
    }
    us.save()?;
    println!(
        "Saved connect defaults to {}",
        UserSettings::path()?.display()
    );
    Ok(())
}

pub async fn run(settings: &Settings, args: &ConnectArgs, verbose: &Verbosity) -> Result<()> {
    require_root_for("run OpenVPN connections", settings.no_elevate)?;

    let us = UserSettings::load();
    let resolved = resolve_connect(&us, args);
    if settings.save_defaults {
        let mut us = UserSettings::load();
        persist_connect_defaults(&mut us, args)?;
    }

    let pool = init_pool(&settings.db_path).await?;
    let repo = Arc::new(ConfigRepo::new(pool));

    let queue = build_queue(settings, args, resolved.cooldown, repo.clone()).await?;

    if queue.is_empty() {
        if settings.filter.is_empty() {
            println!(
                "No connectable configs in history. Run `vmate-cli scan <dir>` to discover and store configs first."
            );
        } else {
            // CLI-friendly comma list without spaces for the suggested flag.
            let filter_arg = settings.filter.to_display().replace(", ", ",");
            println!(
                "No connectable configs matched filter: {}. Run `vmate-cli scan <dir> --filter {filter_arg}` to find matching configs.",
                settings.filter
            );
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
        connect_timeout: resolved.connect_timeout,
        // A session this stable is real: its crash resets the retry budget.
        connect_stability_grace: resolved.stability_grace,
        retry_count: resolved.retry_count,
        killall_enabled: settings.killall_enabled,
    };

    let mut host = crate::ui::connect::ConnectTui::new(
        args.no_interactive,
        settings.filter.to_display(),
        crate::app::is_verbose(verbose),
    )?;
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
    cooldown: Duration,
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
    let stored = repo.connect_candidates(&settings.filter, cooldown).await?;
    for stored_config in stored {
        let candidate = Candidate::from(&stored_config);
        if !candidates.iter().any(|c| c.path == candidate.path) {
            candidates.push(candidate);
        }
    }

    Ok(ConnectQueue::new(candidates))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn args(
        connect_timeout: Option<Duration>,
        cooldown: Option<Duration>,
        retry_count: Option<u32>,
        stability_grace: Option<Duration>,
    ) -> ConnectArgs {
        ConnectArgs {
            config: None,
            connect_timeout,
            strict_filter: false,
            cooldown,
            retry_count,
            stability_grace,
            no_interactive: false,
        }
    }

    fn us_with(
        connect_timeout_secs: Option<u64>,
        cooldown_secs: Option<u64>,
        retry_count: Option<u32>,
        stability_grace_secs: Option<u64>,
    ) -> UserSettings {
        UserSettings {
            max_workers: None,
            limit: None,
            scan_timeout_secs: None,
            connect_timeout_secs,
            cooldown_secs,
            retry_count,
            stability_grace_secs,
        }
    }

    #[test]
    fn resolution_falls_back_to_builtin_defaults() {
        let us = UserSettings::default();
        let r = resolve_connect(&us, &args(None, None, None, None));
        assert_eq!(r.connect_timeout, Duration::from_secs(5));
        assert_eq!(r.cooldown, Duration::from_secs(30));
        assert_eq!(r.retry_count, 2);
        assert_eq!(r.stability_grace, Duration::from_secs(5));
    }

    #[test]
    fn resolution_honors_persisted_values() {
        let us = us_with(Some(7), Some(45), Some(3), Some(9));
        let r = resolve_connect(&us, &args(None, None, None, None));
        assert_eq!(r.connect_timeout, Duration::from_secs(7));
        assert_eq!(r.cooldown, Duration::from_secs(45));
        assert_eq!(r.retry_count, 3);
        assert_eq!(r.stability_grace, Duration::from_secs(9));
    }

    #[test]
    fn resolution_prefers_explicit_flags_over_persisted() {
        let us = us_with(Some(7), Some(45), Some(3), Some(9));
        let r = resolve_connect(
            &us,
            &args(
                Some(Duration::from_secs(2)),
                Some(Duration::from_secs(10)),
                Some(5),
                Some(Duration::from_secs(1)),
            ),
        );
        assert_eq!(r.connect_timeout, Duration::from_secs(2));
        assert_eq!(r.cooldown, Duration::from_secs(10));
        assert_eq!(r.retry_count, 5);
        assert_eq!(r.stability_grace, Duration::from_secs(1));
    }
}

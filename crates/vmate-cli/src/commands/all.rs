//! `vmate-cli all`: scan, store, then connect using only the filtered matches.

use crate::cli::AllArgs;
use crate::commands::connect::{persist_connect_defaults, resolve_connect};
use crate::settings::Settings;
use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use vmate_core::connect::{Candidate, ConnectOptions, ConnectQueue, ConnectService};
use vmate_core::ovpn::process::RealOpenVpnRunner;
use vmate_core::settings::UserSettings;
use vmate_core::system::{ProcessKiller, RealProcessKiller, require_root_for};

pub async fn run(settings: &Settings, args: &AllArgs, verbose: &Verbosity) -> Result<()> {
    // --save-defaults is a pure settings operation: persist and exit without
    // scanning or connecting (no root, no OpenVPN, no DB needed).
    if settings.save_defaults {
        crate::commands::scan::persist_scan_defaults(&args.scan)?;
        persist_connect_defaults(&args.connect)?;
        return Ok(());
    }

    require_root_for("run OpenVPN tests and connections", settings.no_elevate)?;

    let us = UserSettings::load();
    let connect = resolve_connect(&us, &args.connect);

    // The scan preamble (wiring, options, report, export) is shared with
    // `scan`; `all` keeps only the connect half.
    let (report, repo) =
        crate::commands::scan::scan_pipeline(settings, &args.scan, verbose).await?;

    if args.no_connect {
        return Ok(());
    }

    if report.matched_configs.is_empty() {
        println!("No successful configs matched filter: {}", settings.filter);
        return Ok(());
    }

    // Build the connect queue from the fresh scan's filtered matches only.
    let mut candidates = Vec::new();
    for m in &report.matched_configs {
        let id = repo
            .config_by_path(&m.path)
            .await?
            .map(|c| c.id)
            .unwrap_or(0);
        candidates.push(Candidate {
            id,
            path: m.path.to_string_lossy().to_string(),
            country: m.country.to_string(),
        });
    }
    let queue = ConnectQueue::new(candidates);

    let registry = Arc::new(vmate_core::system::ProcessRegistry::new());
    let runner = Arc::new(RealOpenVpnRunner {
        bin: settings.openvpn_bin.clone(),
        registry: registry.clone(),
    });
    let killer: Arc<dyn ProcessKiller> = Arc::new(RealProcessKiller {
        killall_enabled: settings.killall_enabled,
    });
    let options = ConnectOptions {
        connect_timeout: connect.connect_timeout,
        // A session this stable is real: its crash resets the retry budget.
        connect_stability_grace: connect.stability_grace,
        retry_count: connect.retry_count,
        killall_enabled: settings.killall_enabled,
    };

    let mut host = crate::ui::connect::ConnectTui::new(
        args.connect.no_interactive,
        settings.filter.to_display(),
        crate::app::is_verbose(verbose),
    )?;
    let service = ConnectService {
        runner,
        killer,
        registry,
        repo,
        options,
    };
    service.run(queue, &mut host).await?;
    Ok(())
}

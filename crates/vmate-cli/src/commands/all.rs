//! `vmate all`: scan, store, then connect using only the filtered matches.

use crate::cli::AllArgs;
use crate::settings::Settings;
use crate::ui::progress::{ProgressReporter, VerboseReporter};
use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use vmate_core::connect::{Candidate, ConnectOptions, ConnectQueue, ConnectService};
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::geo::IpInfoGeoLocator;
use vmate_core::ovpn::process::{RealOpenVpnRunner, RealVpnTester, VpnTester};
use vmate_core::scan::{ScanOptions, ScanProgress, ScanService};
use vmate_core::system::{
    CleanupGuard, ProcessKiller, RealProcessKiller, require_root_for, shutdown_signal,
};

pub async fn run(settings: &Settings, args: &AllArgs, verbose: &Verbosity) -> Result<()> {
    require_root_for("run OpenVPN tests and connections")?;

    let pool = init_pool(&settings.db_path).await?;
    let repo = Arc::new(ConfigRepo::new(pool));
    let killer: Arc<dyn ProcessKiller> = Arc::new(RealProcessKiller {
        killall_enabled: settings.killall_enabled,
    });
    let tester: Arc<dyn VpnTester> = Arc::new(RealVpnTester {
        bin: settings.openvpn_bin.clone(),
        killer: killer.clone(),
    });
    let geo = Arc::new(IpInfoGeoLocator::new(
        repo.clone(),
        settings.ipinfo_token.clone(),
    ));

    let scan_service = ScanService {
        tester,
        geo,
        repo: repo.clone(),
    };

    let scan_options = ScanOptions {
        dir: args.scan.dir.clone(),
        limit: args.scan.limit,
        timeout: args.scan.timeout,
        workers: args.scan.max,
        modify: args.scan.modify,
        backup: args.scan.backup,
        export: args.scan.export.clone(),
        no_save: args.scan.no_save,
        filter: settings.filter.clone(),
    };

    let cancel = CancellationToken::new();
    let cancel_task = cancel.clone();
    let signal_task = tokio::spawn(async move {
        let _ = shutdown_signal().await;
        cancel_task.cancel();
    });
    let _guard = CleanupGuard::new(killer.clone(), settings.killall_enabled);

    let progress: Arc<dyn ScanProgress> = if crate::app::is_verbose(verbose) {
        Arc::new(VerboseReporter)
    } else {
        Arc::new(ProgressReporter::new(settings.filter.to_display()))
    };

    let report = scan_service.scan(&scan_options, progress, cancel).await?;
    signal_task.abort();

    println!();
    println!("--- Scan Result ---");
    println!("Scanned:  {}", report.scanned);
    println!("Tested:   {}", report.tested);
    println!("Success:  {}", report.success);
    println!("Matched:  {}", report.matched);
    println!("Filter:   {}", report.filter);
    for m in &report.matched_configs {
        println!("{} -- {}", m.country, m.path.display());
    }

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

    let runner = Arc::new(RealOpenVpnRunner {
        bin: settings.openvpn_bin.clone(),
    });
    let options = ConnectOptions {
        connect_timeout: args.connect.connect_timeout,
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
        repo,
        options,
    };
    service.run(queue, &mut host).await?;
    Ok(())
}

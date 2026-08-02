//! `vmate-cli scan`: discover configs, test them concurrently, store and report.

use crate::cli::ScanArgs;
use crate::settings::Settings;
use crate::ui::progress::{ProgressReporter, VerboseReporter};
use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::geo::IpInfoGeoLocator;
use vmate_core::ovpn::process::{RealVpnTester, VpnTester};
use vmate_core::scan::{ScanOptions, ScanProgress, ScanReport, ScanService};
use vmate_core::system::{
    CleanupGuard, ProcessKiller, RealProcessKiller, require_root_for, shutdown_signal,
};

pub async fn run(settings: &Settings, args: &ScanArgs, verbose: &Verbosity) -> Result<()> {
    require_root_for("run OpenVPN tests")?;

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

    let service = ScanService {
        tester,
        geo,
        repo: repo.clone(),
    };

    let options = ScanOptions {
        dir: args.dir.clone(),
        limit: args.limit,
        timeout: args.timeout,
        workers: args.max,
        modify: args.modify,
        backup: args.backup,
        no_save: args.no_save,
        filter: settings.filter.clone(),
    };

    // Ctrl+C / SIGTERM cancels the scan; the cleanup guard kills any stale
    // OpenVPN processes on the way out.
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

    let report = service.scan(&options, progress, cancel).await?;
    signal_task.abort();

    print_report(&report, settings);

    // Export this scan's fresh matches. The scan above still stores successes
    // to the DB (unless --no-save), so `vmate-cli recent` is updated as usual.
    if let Some(export_dir) = &args.export {
        let dest = vmate_core::paths::expand_path(export_dir);
        let result =
            vmate_core::export::export_configs_from_matches(&report.matched_configs, &dest).await?;
        println!(
            "Exported {} of {} configs to {}",
            result.exported,
            result.total,
            result.dest.display()
        );
    }

    Ok(())
}

fn print_report(report: &ScanReport, settings: &Settings) {
    println!();
    println!("--- Final Result ---");
    for m in &report.matched_configs {
        println!("{} -- {}", m.country, m.path.display());
    }
    println!("Found matched: {}", report.matched);
    println!("Found total:   {}", report.success);
    println!("Scanned:       {}", report.scanned);
    println!("Filter:        {}", report.filter);
    if report.saved_to_db {
        println!("Saved to database: {}", settings.db_path.display());
    }
}

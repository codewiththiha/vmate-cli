//! `vmate-cli scan`: discover configs, test them concurrently, store and report.

use crate::cli::ScanArgs;
use crate::settings::Settings;
use crate::ui::progress::{ProgressReporter, VerboseReporter};
use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use vmate_core::db::ConfigRepo;
use vmate_core::db::pool::init_pool;
use vmate_core::geo::IpInfoGeoLocator;
use vmate_core::ovpn::process::{RealVpnTester, VpnTester};
use vmate_core::scan::{ScanOptions, ScanProgress, ScanReport, ScanService};
use vmate_core::settings::UserSettings;
use vmate_core::system::{
    CleanupGuard, ProcessKiller, RealProcessKiller, require_root_for, shutdown_signal,
};

pub async fn run(settings: &Settings, args: &ScanArgs, verbose: &Verbosity) -> Result<()> {
    // --save-defaults is a pure settings operation: persist and exit without
    // scanning (no root, no OpenVPN, no DB needed).
    if settings.save_defaults {
        persist_scan_defaults(args)?;
        return Ok(());
    }

    require_root_for("run OpenVPN tests", settings.no_elevate)?;

    scan_pipeline(settings, args, verbose).await?;
    Ok(())
}

/// The full scan orchestration: DB/repo/killer/tester/geo wiring, option
/// resolution, the scan itself, the report and the export. Returns the report
/// and repo so `all` can go on to connect. Shared by `scan` and `all` so the
/// preamble (and its report format) lives in one place.
pub(crate) async fn scan_pipeline(
    settings: &Settings,
    args: &ScanArgs,
    verbose: &Verbosity,
) -> Result<(ScanReport, Arc<ConfigRepo>)> {
    let us = UserSettings::load();
    let (workers, limit, timeout) = resolve_scan_defaults(args, &us);

    let dir = resolve_scan_dir(args)?;

    let pool = init_pool(&settings.db_path).await?;
    let repo = Arc::new(ConfigRepo::new(pool));
    let registry = Arc::new(vmate_core::system::ProcessRegistry::new());
    let killer: Arc<dyn ProcessKiller> = Arc::new(RealProcessKiller {
        killall_enabled: settings.killall_enabled,
    });
    let tester: Arc<dyn VpnTester> = Arc::new(RealVpnTester {
        bin: settings.openvpn_bin.clone(),
        killer: killer.clone(),
        registry: registry.clone(),
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
        dir,
        limit,
        timeout,
        workers,
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
    let _guard = CleanupGuard::new(killer.clone(), registry.clone(), settings.killall_enabled);

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

    Ok((report, repo))
}

/// Resolve the scan `workers`/`limit`/`timeout` as
/// `explicit CLI flag → persisted setting → built-in default`.
pub(crate) fn resolve_scan_defaults(
    args: &ScanArgs,
    us: &UserSettings,
) -> (usize, usize, Duration) {
    (
        us.max_workers(args.max.map(|v| v as u64)) as usize,
        us.limit(args.limit.map(|v| v as u64)) as usize,
        us.scan_timeout(args.timeout),
    )
}

/// Persist the explicitly-passed scan default flags to the user config and
/// print a confirmation. Only the flags actually passed are written.
pub(crate) fn persist_scan_defaults(args: &ScanArgs) -> Result<()> {
    let mut us = UserSettings::load();
    us.persist_scan(&vmate_core::settings::ScanDefaults {
        max_workers: args.max.map(|v| v as u64),
        limit: args.limit.map(|v| v as u64),
        timeout: args.timeout,
    });
    us.save()?;
    println!("Saved scan defaults to {}", UserSettings::path()?.display());
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

/// The directory to scan: an explicit one, or — when omitted — the built-in
/// directory for the chosen provider + proto, materializing the configs first.
fn resolve_scan_dir(args: &ScanArgs) -> Result<std::path::PathBuf> {
    match &args.dir {
        Some(dir) => Ok(dir.clone()),
        None => materialize_builtins(&args.provider, &args.proto),
    }
}

/// Materialize every built-in config for `provider`/`proto` into
/// `builtin_dir/<provider>/<proto>/` and return that directory. Used by both
/// `scan` and `all` when no explicit directory is given.
pub fn materialize_builtins(provider: &str, proto: &str) -> Result<std::path::PathBuf> {
    let provider = vmate_core::builtin::Provider::from_name(provider)
        .ok_or_else(|| anyhow::anyhow!("unknown provider '{}'; available: vpn-gate", provider))?;
    let proto = vmate_core::builtin::Proto::from_name(proto)
        .ok_or_else(|| anyhow::anyhow!("invalid proto '{}'; use udp or tcp", proto))?;
    let dir = vmate_core::paths::builtin_dir()?
        .join(provider.name())
        .join(proto.as_str());
    let count = materialize_builtins_into(provider, proto, &dir)?;
    println!(
        "Scanning {} built-in configs ({} remotes, proto {})",
        provider.name(),
        count,
        proto.as_str()
    );
    Ok(dir)
}

/// Write one `.ovpn` file per built-in config into `dir` and return the number
/// written. Idempotent: existing files are overwritten.
fn materialize_builtins_into(
    provider: vmate_core::builtin::Provider,
    proto: vmate_core::builtin::Proto,
    dir: &std::path::Path,
) -> Result<usize> {
    let configs = vmate_core::builtin::enumerate(provider, proto);
    std::fs::create_dir_all(dir)?;
    for cfg in &configs {
        vmate_core::builtin::materialize(cfg, dir)?;
    }
    Ok(configs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialize_builtins_into_writes_one_file_per_remote() {
        let dir = tempfile::tempdir().unwrap();
        let provider = vmate_core::builtin::Provider::VpnGate;
        let proto = vmate_core::builtin::Proto::Udp;
        let expected = vmate_core::builtin::enumerate(provider, proto).len();
        let count = materialize_builtins_into(provider, proto, dir.path()).unwrap();
        assert_eq!(count, expected);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), expected);

        // A known remote maps to the expected file with the built config.
        let expected_path = dir.path().join("public-vpn-38.opengw.net-1195.ovpn");
        let cfg = vmate_core::builtin::BuiltinConfig {
            provider,
            remote: "remote public-vpn-38.opengw.net 1195".to_string(),
            proto,
        };
        assert_eq!(
            std::fs::read_to_string(&expected_path).unwrap(),
            vmate_core::builtin::build_config(&cfg)
        );
    }

    #[test]
    fn materialize_builtins_into_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let provider = vmate_core::builtin::Provider::VpnGate;
        let proto = vmate_core::builtin::Proto::Tcp;
        let first = materialize_builtins_into(provider, proto, dir.path()).unwrap();
        let second = materialize_builtins_into(provider, proto, dir.path()).unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), first);
    }

    #[test]
    fn materialize_builtins_rejects_unknown_provider_and_proto() {
        assert!(materialize_builtins("nordvpn", "udp").is_err());
        assert!(materialize_builtins("vpn-gate", "quic").is_err());
    }

    fn scan_args(max: Option<usize>, limit: Option<usize>, timeout: Option<Duration>) -> ScanArgs {
        ScanArgs {
            dir: None,
            provider: "vpn-gate".to_string(),
            proto: "udp".to_string(),
            max,
            limit,
            timeout,
            modify: false,
            backup: false,
            export: None,
            no_save: true,
        }
    }

    #[test]
    fn resolve_scan_defaults_falls_back_to_builtins() {
        let us = UserSettings::default();
        let args = scan_args(None, None, None);
        let (workers, limit, timeout) = resolve_scan_defaults(&args, &us);
        assert_eq!(
            (workers, limit, timeout),
            (64, 100, Duration::from_secs(15))
        );
    }

    #[test]
    fn resolve_scan_defaults_prefers_persisted_over_builtin() {
        let us = UserSettings {
            max_workers: Some(128),
            limit: Some(50),
            scan_timeout_secs: Some(20),
            ..Default::default()
        };
        let args = scan_args(None, None, None);
        let (workers, limit, timeout) = resolve_scan_defaults(&args, &us);
        assert_eq!(
            (workers, limit, timeout),
            (128, 50, Duration::from_secs(20))
        );
    }

    #[test]
    fn resolve_scan_defaults_explicit_flags_win() {
        let us = UserSettings {
            max_workers: Some(128),
            limit: Some(50),
            scan_timeout_secs: Some(20),
            ..Default::default()
        };
        let args = scan_args(Some(500), Some(10), Some(Duration::from_secs(30)));
        let (workers, limit, timeout) = resolve_scan_defaults(&args, &us);
        assert_eq!(
            (workers, limit, timeout),
            (500, 10, Duration::from_secs(30))
        );
    }

    #[test]
    fn persist_scan_only_writes_explicitly_passed_keys() {
        use vmate_core::settings::ScanDefaults;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let mut us = UserSettings::default();
        us.persist_scan(&ScanDefaults {
            max_workers: Some(500),
            limit: None,
            timeout: Some(Duration::from_secs(20)),
        });
        us.save_to(&path).unwrap();

        let loaded = UserSettings::load_from(&path);
        assert_eq!(loaded.max_workers, Some(500));
        assert_eq!(loaded.limit, None);
        assert_eq!(loaded.scan_timeout_secs, Some(20));

        // A later save with a different explicit key must not resurrect keys
        // that were never passed.
        let mut us2 = loaded;
        us2.persist_scan(&ScanDefaults {
            max_workers: None,
            limit: Some(25),
            timeout: None,
        });
        us2.save_to(&path).unwrap();
        let loaded2 = UserSettings::load_from(&path);
        assert_eq!(loaded2.max_workers, Some(500));
        assert_eq!(loaded2.limit, Some(25));
        assert_eq!(loaded2.scan_timeout_secs, Some(20));
    }
}

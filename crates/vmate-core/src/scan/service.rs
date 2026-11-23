//! The scan service: concurrent testing with a semaphore, filtered match
//! limiting and cancellation.

use crate::country::CountryCode;
use crate::db::ConfigRepo;
use crate::db::models::CountrySource;
use crate::geo::{CountryLookup, GeoLocator};
use crate::ovpn::cipher::modify_config_cipher;
use crate::ovpn::parser::parse_remote_host;
use crate::ovpn::process::{VpnTester, discover_configs};
use crate::paths;
use crate::scan::report::{ScanMatch, ScanOptions, ScanProgress, ScanReport};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// The production scan service.
pub struct ScanService {
    pub tester: Arc<dyn VpnTester>,
    pub geo: Arc<dyn GeoLocator>,
    pub repo: Arc<ConfigRepo>,
}

#[derive(Default)]
struct Counts {
    tested: AtomicUsize,
    ok: AtomicUsize,
    matched: AtomicUsize,
}

impl ScanService {
    /// Run a scan.
    ///
    /// All configs may be tested, but only successful configs whose country
    /// matches the filter count toward `options.limit` and appear in the final
    /// report. Unfiltered successes are still stored in the database — the
    /// filter limits what is *shown*, not what is *learned*.
    pub async fn scan(
        &self,
        options: &ScanOptions,
        progress: Arc<dyn ScanProgress>,
        cancel: CancellationToken,
    ) -> Result<ScanReport> {
        let paths = prepare_paths(options)?;
        let total = paths.len();
        progress.total(total);

        let cancel = cancel.child_token();
        let workers = super::resources::workers(options.workers);
        let semaphore = Arc::new(Semaphore::new(workers));
        let counts = Arc::new(Counts::default());
        let results: Arc<Mutex<Vec<ScanMatch>>> = Arc::new(Mutex::new(Vec::new()));
        let mut join_set = JoinSet::new();
        let fatal = Arc::new(Mutex::new(None));

        for path in paths {
            if cancel.is_cancelled() {
                break;
            }
            while let Some(joined) = join_set.try_join_next() {
                joined.context("scan worker panicked")?;
            }
            let permit = tokio::select! {
                _ = cancel.cancelled() => break,
                permit = semaphore.clone().acquire_owned() => permit?,
            };

            let fatal = fatal.clone();
            let tester = self.tester.clone();
            let geo = self.geo.clone();
            let repo = self.repo.clone();
            let filter = options.filter.clone();
            let results = results.clone();
            let counts = counts.clone();
            let progress = progress.clone();
            let cancel = cancel.clone();
            let limit = options.limit;
            let no_save = options.no_save;
            let timeout = options.timeout;

            join_set.spawn(async move {
                let _permit = permit;
                if cancel.is_cancelled() {
                    return;
                }

                let ok = match tester.test(&path, timeout, cancel.clone()).await {
                    Ok(v) => v,
                    Err(e) => {
                        fatal.lock().get_or_insert(e);
                        cancel.cancel();
                        return;
                    }
                };

                counts.tested.fetch_add(1, Ordering::SeqCst);
                progress.tested();

                if !ok {
                    if cancel.is_cancelled() {
                        return;
                    }
                    progress.failed(&path);
                    if !no_save {
                        let _ = repo.record_failure(&path, "connection test failed").await;
                    }
                    return;
                }

                counts.ok.fetch_add(1, Ordering::SeqCst);
                progress.ok();

                let Some(lookup) = lookup_country(geo.as_ref(), &path, &cancel).await else {
                    return;
                };

                if !no_save {
                    let sha = crate::hash::sha256_file(&path)
                        .unwrap_or_else(|_| crate::hash::sha256_str(&path.to_string_lossy()));
                    let remote_host = parse_remote_host(&path).ok();
                    let _ = repo
                        .record_success(
                            &path,
                            &sha,
                            remote_host.as_deref(),
                            &lookup.country,
                            lookup.source,
                        )
                        .await;
                }

                progress.success(&path, &lookup.country);

                if filter.matches(lookup.country.as_str()) {
                    let mut results = results.lock();
                    if results.len() < limit {
                        results.push(ScanMatch {
                            path,
                            country: lookup.country,
                        });
                        counts.matched.fetch_add(1, Ordering::SeqCst);
                        progress.matched();
                        if results.len() >= limit {
                            cancel.cancel();
                        }
                    }
                }
            });
        }

        while let Some(joined) = join_set.join_next().await {
            if let Err(e) = joined {
                tracing::debug!(error = %e, "scan worker task failed");
            }
        }

        progress.finish();
        if let Some(error) = fatal.lock().take() {
            return Err(error);
        }
        Ok(finish_report(options, total, &counts, &results))
    }
}

fn backup_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".bak");
    PathBuf::from(s)
}

fn prepare_paths(options: &ScanOptions) -> Result<Vec<PathBuf>> {
    let dir = paths::canonicalize_dir(&options.dir)
        .with_context(|| format!("scan directory does not exist: {}", options.dir.display()))?;
    let paths = discover_configs(&dir)?;
    for path in &paths {
        crate::ovpn::policy::validate(path, true)?;
    }
    if options.modify {
        for path in &paths {
            if options.backup
                && let Err(error) = std::fs::copy(path, backup_path(path))
            {
                tracing::warn!(path = %path.display(), %error, "backup failed");
            }
            if let Err(error) = modify_config_cipher(path) {
                tracing::warn!(path = %path.display(), %error, "cipher modification failed");
            }
        }
    }
    Ok(paths)
}

async fn lookup_country(
    geo: &dyn GeoLocator,
    path: &Path,
    cancel: &CancellationToken,
) -> Option<CountryLookup> {
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => return None,
        result = tokio::time::timeout(std::time::Duration::from_secs(5), geo.country_for_config(path)) => result,
    };
    match result {
        Ok(Ok(lookup)) => Some(lookup),
        _ => Some(CountryLookup {
            country: CountryCode::unknown(),
            source: CountrySource::Unknown,
        }),
    }
}

fn finish_report(
    options: &ScanOptions,
    total: usize,
    counts: &Counts,
    results: &Mutex<Vec<ScanMatch>>,
) -> ScanReport {
    let mut matched_configs = std::mem::take(&mut *results.lock());
    matched_configs.sort_by(|a, b| a.path.cmp(&b.path));
    ScanReport {
        scanned: total,
        tested: counts.tested.load(Ordering::SeqCst),
        success: counts.ok.load(Ordering::SeqCst),
        matched: matched_configs.len(),
        matched_configs,
        saved_to_db: !options.no_save,
        filter: options.filter.to_display(),
    }
}

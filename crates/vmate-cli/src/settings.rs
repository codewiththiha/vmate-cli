//! Resolution of runtime settings from parsed CLI arguments.

use crate::cli::Cli;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use vmate_core::filter::CountryFilter;
use vmate_core::paths;

/// Effective configuration for a run.
#[derive(Debug, Clone)]
pub struct Settings {
    pub db_path: PathBuf,
    pub openvpn_bin: String,
    pub killall_enabled: bool,
    pub ipinfo_token: Option<String>,
    pub filter: CountryFilter,
}

impl Settings {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let filter = CountryFilter::from_args(&cli.filter).map_err(|e| {
            anyhow!(
                "invalid --filter value: {e}\n\
                 hint: country codes should be two letters, e.g. JP, KR, US, or UNKNOWN"
            )
        })?;

        let db_path = match &cli.db {
            Some(path) => paths::expand_path(path),
            None => paths::default_db_path()?,
        };

        Ok(Settings {
            db_path,
            openvpn_bin: cli.openvpn_bin.clone(),
            killall_enabled: !cli.no_killall,
            ipinfo_token: cli.ipinfo_token.clone(),
            filter,
        })
    }
}

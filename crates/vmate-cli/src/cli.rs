//! `clap` command-line interface definition.

use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "vmate-cli",
    version,
    about = "OpenVPN config scanner, tester and connector",
    long_about = None,
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub verbose: Verbosity,

    /// Filter by country code. Case-insensitive.
    ///
    /// Examples:
    ///   --filter JP,KR
    ///   --filter jp -f kr
    #[arg(
        long,
        short = 'f',
        global = true,
        value_delimiter = ',',
        value_name = "COUNTRY"
    )]
    pub filter: Vec<String>,

    /// Path to the SQLite database.
    #[arg(long, global = true, env = "VMATE_DB")]
    pub db: Option<PathBuf>,

    /// OpenVPN binary to use.
    #[arg(
        long,
        global = true,
        default_value = "openvpn",
        env = "VMATE_OPENVPN_BIN"
    )]
    pub openvpn_bin: String,

    /// Also run `killall -9 openvpn` on shutdown/switch. Default is per-process
    /// cleanup of only the openvpn processes vmate spawned.
    #[arg(long, global = true)]
    pub killall: bool,

    /// Don't re-execute under sudo; run with current privileges (OpenVPN will likely fail).
    #[arg(long, global = true)]
    pub no_elevate: bool,

    /// ipinfo.io API token.
    #[arg(long, global = true, env = "IPINFO_TOKEN")]
    pub ipinfo_token: Option<String>,

    /// Save explicitly-passed default flags (e.g. --max, --timeout, --retry-count)
    /// to the persistent config for future sessions.
    #[arg(long, global = true)]
    pub save_defaults: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan a directory and test OpenVPN configs.
    Scan(ScanArgs),

    /// Connect to a config, retrying intelligently.
    Connect(ConnectArgs),

    /// Show previously successful configs.
    Recent(RecentArgs),

    /// Scan, store results, then connect using stored results.
    All(AllArgs),

    /// Export successful configs.
    Export(ExportArgs),

    /// Check dependencies and environment.
    Doctor,

    /// Install shell completions (or print the script with --print).
    Completions(CompletionsArgs),
}

#[derive(clap::Args)]
pub struct ScanArgs {
    /// Directory containing .ovpn files. When omitted, scans the built-in
    /// remotes for the configured provider (see --provider and --proto).
    pub dir: Option<PathBuf>,

    /// Built-in provider to scan when no directory is given.
    #[arg(long, default_value = "vpn-gate")]
    pub provider: String,

    /// Transport protocol used to build the built-in configs.
    #[arg(long, default_value = "udp", value_parser = ["udp", "tcp"])]
    pub proto: String,

    /// Maximum number of matched successful configs to collect.
    ///
    /// If --filter is used, this limit applies to matched filtered results.
    #[arg(long, short = 'l')]
    pub limit: Option<usize>,

    /// Timeout for each OpenVPN test.
    #[arg(long, short = 't', value_parser = parse_duration)]
    pub timeout: Option<std::time::Duration>,

    /// Maximum concurrent OpenVPN test processes.
    #[arg(long, short = 'm')]
    pub max: Option<usize>,

    /// Modify outdated cipher lines before testing.
    #[arg(long)]
    pub modify: bool,

    /// Back up modified configs to `.bak` files.
    #[arg(long, requires = "modify")]
    pub backup: bool,

    /// Export matched successful configs to this directory.
    #[arg(long)]
    pub export: Option<PathBuf>,

    /// Do not save results to the database.
    #[arg(long)]
    pub no_save: bool,
}

#[derive(clap::Args)]
pub struct ConnectArgs {
    /// Optional explicit config to connect to.
    pub config: Option<PathBuf>,

    /// Timeout for the initial connection handshake.
    #[arg(
        long,
        default_value = "5s",
        value_parser = parse_duration
    )]
    pub connect_timeout: std::time::Duration,

    /// If an explicit config does not match --filter, reject it.
    ///
    /// By default an explicit config is attempted even if it does not match
    /// the filter; retry/fallback candidates still respect the filter.
    #[arg(long)]
    pub strict_filter: bool,

    /// Cooldown before retrying a config that recently failed.
    #[arg(
        long,
        default_value = "30s",
        value_parser = parse_duration
    )]
    pub cooldown: std::time::Duration,

    /// Disable interactive key handling.
    #[arg(long)]
    pub no_interactive: bool,
}

#[derive(clap::Args)]
pub struct RecentArgs {
    /// Maximum number of entries to show.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Show all entries.
    #[arg(long)]
    pub all: bool,

    /// Disable the TUI and print a plain table.
    #[arg(long)]
    pub no_tui: bool,

    /// Copy the first entry's path immediately.
    #[arg(long)]
    pub copy_first: bool,

    /// Also copy the listed configs to this directory.
    #[arg(long)]
    pub export: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct AllArgs {
    /// Scan options.
    #[command(flatten)]
    pub scan: ScanArgs,

    /// Connect options.
    #[command(flatten)]
    pub connect: ConnectArgs,

    /// Do not automatically connect after scanning.
    #[arg(long)]
    pub no_connect: bool,
}

#[derive(clap::Args)]
pub struct ExportArgs {
    /// Destination directory for exported configs.
    #[arg(long, short = 'o', default_value = "./exported")]
    pub out: PathBuf,
}

#[derive(clap::Args)]
pub struct CompletionsArgs {
    /// Shell to install completions for.
    pub shell: clap_complete::Shell,

    /// Print the completion script to stdout instead of installing it.
    #[arg(long)]
    pub print: bool,
}

/// Parse a duration for `--timeout` and friends.
///
/// Bare numbers are treated as seconds to stay compatible with the original
/// Go tool (`--timeout 15`), while `5s`, `500ms`, etc. are also accepted via
/// humantime.
fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    if let Ok(secs) = s.parse::<u64>() {
        return Ok(std::time::Duration::from_secs(secs));
    }
    humantime::parse_duration(s).map_err(|e| e.to_string())
}

//! Top-level application wiring: logging, settings, subcommand dispatch.

use crate::cli::{Cli, Command};
use crate::settings::Settings;
use anyhow::Result;
use clap_verbosity_flag::Verbosity;
use clap_verbosity_flag::log::LevelFilter;
use tracing_subscriber::EnvFilter;

pub async fn run(cli: Cli) -> Result<()> {
    init_tracing(&cli.verbose);
    let settings = Settings::from_cli(&cli)?;

    match &cli.command {
        Command::Scan(args) => crate::commands::scan::run(&settings, args, &cli.verbose).await,
        Command::Connect(args) => {
            crate::commands::connect::run(&settings, args, &cli.verbose).await
        }
        Command::Recent(args) => crate::commands::recent::run(&settings, args).await,
        Command::All(args) => crate::commands::all::run(&settings, args, &cli.verbose).await,
        Command::Export(args) => crate::commands::export::run(&settings, args).await,
        Command::Doctor => crate::commands::doctor::run(&settings).await,
        Command::Completions(args) => crate::commands::completions::run(args.shell, args.install),
    }
}

/// Whether the user asked for verbose (`-v` or higher) output.
pub(crate) fn is_verbose(verbose: &Verbosity) -> bool {
    verbose.log_level_filter() >= LevelFilter::Warn
}

/// Initialize tracing, writing to stderr so stdout stays clean for tables,
/// JSON and TUIs. Uses `try_init` so tests can install their own subscriber.
fn init_tracing(verbose: &Verbosity) {
    let filter = EnvFilter::new(verbose.log_level_filter().to_string().to_lowercase());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

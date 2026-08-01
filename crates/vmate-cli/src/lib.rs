//! vmate-cli — the command line application.
//!
//! This crate is a thin shell over `vmate-core`: it parses arguments with
//! `clap`, renders progress bars and TUIs, copies to the clipboard, and maps
//! user-visible errors onto friendly messages.

pub mod app;
pub mod cli;
pub mod commands;
pub mod settings;
pub mod ui;

use clap::Parser;

/// Entry point used by the binary; returns a user-facing error on failure.
pub async fn entry() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    app::run(cli).await
}

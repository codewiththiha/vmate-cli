//! `vmate-cli completions`: generate shell completions.

use crate::cli::Cli;
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "vmate-cli", &mut std::io::stdout());
    Ok(())
}

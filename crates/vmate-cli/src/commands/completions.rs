//! `vmate-cli completions`: generate or install shell completions.

use crate::cli::Cli;
use anyhow::{Result, bail};
use clap::CommandFactory;
use clap_complete::Shell;
use std::fs;
use std::path::{Path, PathBuf};

/// Homebrew `zsh-completions` dirs. macOS zsh setups commonly put one of these
/// on `$fpath` and run `compinit`, so dropping `_vmate-cli` here activates
/// completion with no shell-config edits.
const BREW_ZSH_COMPLETIONS_DIRS: [&str; 2] = [
    "/opt/homebrew/share/zsh-completions",
    "/usr/local/share/zsh-completions",
];

/// Shell completion subcommand: print the script to stdout, or `--install` it.
pub fn run(shell: Shell, install: bool) -> Result<()> {
    let mut cmd = Cli::command();
    if install {
        install_completions(shell, &mut cmd)?;
    } else {
        clap_complete::generate(shell, &mut cmd, "vmate-cli", &mut std::io::stdout());
    }
    Ok(())
}

/// Resolve the user's home directory, preferring `$HOME` so tests can point
/// it at a temp dir.
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Write the completion script for `shell` to its standard location and print
/// activation steps.
fn install_completions(shell: Shell, cmd: &mut clap::Command) -> Result<()> {
    install_completions_to(shell, cmd, &home_dir())?;
    Ok(())
}

/// The install logic, factored with an injectable home so tests can use a temp
/// dir. Returns the path the script was written to.
fn install_completions_to(shell: Shell, cmd: &mut clap::Command, home: &Path) -> Result<PathBuf> {
    let (dest, zsh_fallback) = match shell {
        Shell::Zsh => zsh_target(home, &BREW_ZSH_COMPLETIONS_DIRS),
        Shell::Bash => (
            home.join(".local/share/bash-completion/completions/vmate-cli"),
            false,
        ),
        Shell::Fish => (home.join(".config/fish/completions/vmate-cli.fish"), false),
        other => bail!(
            "automatic install is not supported for {other}; run `vmate-cli completions {other} > file` to capture the script"
        ),
    };

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buf = Vec::new();
    clap_complete::generate(shell, cmd, "vmate-cli", &mut buf);
    fs::write(&dest, buf)?;

    println!(
        "Installed {shell} completion for vmate-cli to {}",
        dest.display()
    );
    match shell {
        Shell::Zsh if zsh_fallback => println!(
            "Add to ~/.zshrc before compinit:  fpath=(~/.zfunc $fpath)  then restart your shell."
        ),
        Shell::Zsh => println!("Restart your shell (or run `compinit`) to activate."),
        Shell::Bash => println!(
            "Restart your shell (or run `source ~/.bashrc`). If bash-completion is not \
             installed, add to ~/.bashrc:  source <(vmate-cli completions bash)"
        ),
        Shell::Fish => println!("Restart fish to activate."),
        _ => {}
    }
    Ok(dest)
}

/// Where to install the zsh completion: the first existing Homebrew
/// `zsh-completions` dir (already on `fpath`), else `~/.zfunc/_vmate-cli`.
/// Returns the destination and whether it fell back to `~/.zfunc`.
fn zsh_target(home: &Path, fpath_candidates: &[&str]) -> (PathBuf, bool) {
    for dir in fpath_candidates {
        if Path::new(dir).is_dir() {
            return (Path::new(dir).join("_vmate-cli"), false);
        }
    }
    (home.join(".zfunc/_vmate-cli"), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn build_cmd() -> clap::Command {
        Cli::command()
    }

    #[test]
    fn bash_installs_to_bash_completion_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dest = install_completions_to(Shell::Bash, &mut build_cmd(), dir.path()).unwrap();
        assert_eq!(
            dest,
            dir.path()
                .join(".local/share/bash-completion/completions/vmate-cli")
        );
        assert!(fs::read_to_string(&dest).unwrap().contains("vmate-cli"));
    }

    #[test]
    fn fish_installs_to_fish_completions_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dest = install_completions_to(Shell::Fish, &mut build_cmd(), dir.path()).unwrap();
        assert_eq!(
            dest,
            dir.path().join(".config/fish/completions/vmate-cli.fish")
        );
        assert!(fs::read_to_string(&dest).unwrap().contains("vmate-cli"));
    }

    #[test]
    fn zsh_uses_existing_fpath_dir_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let fpath = dir.path().join("zsh-completions");
        fs::create_dir_all(&fpath).unwrap();
        let (dest, fallback) = zsh_target(dir.path(), &[fpath.to_str().unwrap()]);
        assert_eq!(dest, fpath.join("_vmate-cli"));
        assert!(!fallback);
    }

    #[test]
    fn zsh_falls_back_to_zfunc() {
        let dir = tempfile::tempdir().unwrap();
        let (dest, fallback) = zsh_target(dir.path(), &[]);
        assert_eq!(dest, dir.path().join(".zfunc/_vmate-cli"));
        assert!(fallback);

        // The full install then writes the file and the parent dir.
        let dest = install_completions_to(Shell::Zsh, &mut build_cmd(), dir.path()).unwrap();
        assert!(fs::read_to_string(&dest).unwrap().contains("compdef"));
    }
}

//! Control an existing NetworkManager VPN without elevating the CLI.

use crate::connect::{Candidate, ConnectHost, ConnectionStatus, UserCommand};
use anyhow::{Context, Result, bail};
use std::process::Stdio;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Activate one existing VPN profile until quit, reconnect, or shutdown.
pub async fn run(uuid: &str, host: &mut dyn ConnectHost, cancel: CancellationToken) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("managed requires Linux NetworkManager");
    }
    validate_uuid(uuid)?;
    let kind = nm(&["-g", "connection.type", "connection", "show", "uuid", uuid]).await?;
    if kind.trim() != "vpn" {
        bail!("managed accepts only an existing NetworkManager VPN profile");
    }
    if active(uuid).await? {
        bail!("profile is already active; refusing to take ownership of another session");
    }
    let mut lease = ProfileLease {
        uuid,
        closed: false,
    };
    let mut established = false;
    let result = session(uuid, host, &cancel, &mut established).await;
    let cleanup = teardown(uuid, established).await;
    lease.closed = cleanup.is_ok();
    cleanup.context("NetworkManager did not confirm VPN teardown")?;
    result
}

async fn session(
    uuid: &str,
    host: &mut dyn ConnectHost,
    cancel: &CancellationToken,
    established: &mut bool,
) -> Result<()> {
    loop {
        *established = false;
        status(uuid, false, host).await?;
        let arguments = ["connection", "up", "uuid", uuid];
        let activation = nm(&arguments);
        tokio::pin!(activation);
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Ok(()),
                command = host.poll_command() => {
                    if command == Some(UserCommand::Quit) { return Ok(()); }
                }
                result = &mut activation => { result?; break; }
            }
        }
        *established = true;
        status(uuid, true, host).await?;
        if !connected(uuid, host, cancel).await? {
            return Ok(());
        }
        host.notify("Disconnecting managed VPN...").await?;
        nm(&["connection", "down", "uuid", uuid]).await?;
    }
}

async fn connected(
    uuid: &str,
    host: &mut dyn ConnectHost,
    cancel: &CancellationToken,
) -> Result<bool> {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Ok(false),
            command = host.poll_command() => match command {
                Some(UserCommand::Quit) => return Ok(false),
                Some(UserCommand::Reconnect) => return Ok(true),
                Some(UserCommand::CopyPath) => host.copy(uuid).await?,
                _ => {}
            },
            _ = tick.tick() => {
                if !active(uuid).await? {
                    host.notify("NetworkManager VPN disconnected").await?;
                    return Ok(false);
                }
            }
        }
    }
}

async fn teardown(uuid: &str, established: bool) -> Result<()> {
    if established && !active(uuid).await? {
        return Ok(());
    }
    let arguments = ["connection", "down", "uuid", uuid];
    let down = nm(&arguments);
    tokio::pin!(down);
    let observed = async {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if !active(uuid).await? {
                return Ok(());
            }
        }
    };
    tokio::select! {
        biased;
        result = &mut down => {
            if !active(uuid).await? { Ok(()) }
            else { result?; bail!("NetworkManager did not confirm VPN teardown"); }
        }
        result = observed, if established => result,
        _ = tokio::time::sleep(Duration::from_secs(20)) => {
            bail!("NetworkManager VPN teardown exceeded 20 seconds")
        }
    }
}

async fn status(uuid: &str, connected: bool, host: &mut dyn ConnectHost) -> Result<()> {
    host.status(&ConnectionStatus {
        connected,
        candidate: Some(Candidate {
            id: 0,
            path: uuid.into(),
            country: "VPN".into(),
        }),
        message: if connected {
            "Connected through NetworkManager"
        } else {
            "Activating NetworkManager VPN"
        }
        .into(),
        filter: String::new(),
    })
    .await
}

async fn active(uuid: &str) -> Result<bool> {
    let profiles = nm(&["-g", "UUID", "connection", "show", "--active"]).await?;
    Ok(profiles.lines().any(|line| line.trim() == uuid))
}

async fn nm(args: &[&str]) -> Result<String> {
    let mut command = tokio::process::Command::new("nmcli");
    command
        .args(["--wait", "15"])
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .context("nmcli exceeded its deadline")?
        .context("install NetworkManager and its OpenVPN plugin to use managed")?;
    if !output.status.success() {
        bail!(
            "NetworkManager: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn validate_uuid(uuid: &str) -> Result<()> {
    let valid = uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, value)| {
            if [8, 13, 18, 23].contains(&index) {
                value == b'-'
            } else {
                value.is_ascii_hexdigit()
            }
        });
    if !valid {
        bail!("provide a VPN UUID from `nmcli -f NAME,UUID,TYPE connection show`");
    }
    Ok(())
}

struct ProfileLease<'a> {
    uuid: &'a str,
    closed: bool,
}

impl Drop for ProfileLease<'_> {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        let child = std::process::Command::new("nmcli")
            .args(["--wait", "3", "connection", "down", "uuid", self.uuid])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            let deadline = std::time::Instant::now() + Duration::from_secs(4);
            while matches!(child.try_wait(), Ok(None)) && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_arguments_cannot_be_command_options() {
        assert!(validate_uuid("01234567-89ab-cdef-0123-456789abcdef").is_ok());
        for value in ["--help", "vpn name", "01234567-89ab-cdef-0123-456789abcdeg"] {
            assert!(validate_uuid(value).is_err());
        }
    }
}

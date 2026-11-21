//! Reject config directives that escape supervised process ownership.

use anyhow::{Context, Result, bail};
use std::path::Path;

/// Validate trusted configs for foreground operation and isolated probes.
///
/// Includes, plugins and process/output redirection are rejected, not expanded.
/// This conservative check is not a sandbox for hostile OpenVPN input.
pub fn validate(config: &Path, probe: bool) -> Result<()> {
    let content = std::fs::read_to_string(config)
        .with_context(|| format!("cannot read config {}", config.display()))?;
    validate_content(&content, probe)
}

fn validate_content(content: &str, probe: bool) -> Result<()> {
    for (index, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with(['#', ';']) || line.is_empty() {
            continue;
        }
        let directive = line.split_whitespace().next().unwrap_or("");
        let directive = directive.trim_start_matches("--").trim_matches(['\'', '"']);
        if raw
            .split_whitespace()
            .next()
            .unwrap_or("")
            .contains(['\\', '\'', '"'])
        {
            bail!(
                "quoted or escaped directive names are unsupported on line {}",
                index + 1
            );
        }
        let supervised = [
            "config",
            "daemon",
            "log",
            "log-append",
            "syslog",
            "plugin",
            "writepid",
            "status",
            "management",
            "management-client",
            "chroot",
            "cd",
        ];
        let probe_only = [
            "dev-type",
            "dev-node",
            "up",
            "down",
            "route-up",
            "route-pre-down",
            "ipchange",
            "tls-verify",
            "auth-user-pass-verify",
            "client-connect",
            "client-disconnect",
            "learn-address",
            "iproute",
        ];
        if supervised.contains(&directive) || (probe && probe_only.contains(&directive)) {
            bail!(
                "unsupported supervised config directive '{directive}' on line {}",
                index + 1
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_detachment_includes_and_plugins() {
        for directive in [
            "daemon",
            "--daemon",
            "'daemon'",
            "config",
            "plugin",
            "log",
            "management",
        ] {
            assert!(validate_content(&format!("{directive} argument"), false).is_err());
        }
    }

    #[test]
    fn probes_reject_device_overrides_and_scripts() {
        for directive in [
            "dev-type",
            "dev-node",
            "up",
            "down",
            "route-up",
            "tls-verify",
        ] {
            assert!(validate_content(&format!("{directive} argument"), true).is_err());
        }
        assert!(validate_content("client\ndev tun\nremote example.com 1194\n", true).is_ok());
        assert!(validate_content("# daemon\n; plugin\n<ca>\nPEM\n</ca>\n", true).is_ok());
    }
}

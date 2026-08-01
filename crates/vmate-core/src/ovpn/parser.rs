//! Minimal `.ovpn` config parser.

use crate::error::ParseError;
use std::path::Path;

/// Extract the hostname from the first valid `remote` directive.
///
/// Lines starting with `#` or `;` are comments and skipped. CRLF is handled
/// transparently by `lines()`.
pub fn parse_remote_host(config: &Path) -> Result<String, ParseError> {
    let content = std::fs::read_to_string(config)?;
    if content.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line == "remote" {
            // Not enough tokens, keep looking for a valid remote.
            continue;
        }
        if let Some(rest) = line.strip_prefix("remote") {
            // Ensure "remote" is a full token, e.g. not "remotely".
            let is_remote_directive = rest.starts_with(char::is_whitespace);
            if !is_remote_directive {
                continue;
            }
            let mut parts = line.split_whitespace();
            let _ = parts.next(); // "remote"
            if let Some(host) = parts.next() {
                return Ok(host.to_string());
            }
        }
    }

    Err(ParseError::NoRemote)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(body: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f
    }

    #[test]
    fn extracts_remote_host() {
        let f = write_config("client\nremote some.host.com 1194 udp\ndev tun\n");
        assert_eq!(parse_remote_host(f.path()).unwrap(), "some.host.com");
    }

    #[test]
    fn ignores_comments_and_empty_lines() {
        let f = write_config("# comment\n\n; another\nclient\nremote vpn.example.net 443 tcp\n");
        assert_eq!(parse_remote_host(f.path()).unwrap(), "vpn.example.net");
    }

    #[test]
    fn supports_crlf() {
        let f = write_config("client\r\nremote crlf.example.com 1194 udp\r\n");
        assert_eq!(parse_remote_host(f.path()).unwrap(), "crlf.example.com");
    }

    #[test]
    fn picks_first_remote() {
        let f =
            write_config("remote first.example.com 1194 udp\nremote second.example.com 1194 udp\n");
        assert_eq!(parse_remote_host(f.path()).unwrap(), "first.example.com");
    }

    #[test]
    fn errors_when_no_remote() {
        let f = write_config("client\ndev tun\n");
        assert!(matches!(
            parse_remote_host(f.path()),
            Err(ParseError::NoRemote)
        ));
    }

    #[test]
    fn does_not_match_remote_substrings() {
        let f = write_config("remoteness is not a directive\n");
        assert!(matches!(
            parse_remote_host(f.path()),
            Err(ParseError::NoRemote)
        ));
    }
}

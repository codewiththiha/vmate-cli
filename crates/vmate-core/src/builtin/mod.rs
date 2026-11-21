//! Built-in VPN provider configurations.
//!
//! Free VPN providers (e.g. VPN Gate) publish hundreds of `.ovpn` files that
//! differ only in the `remote` line; the header and the CA/client cert/key are
//! identical. Instead of shipping or scanning hundreds of files, this module
//! hardcodes the shared key once plus a list of remotes, and can build and
//! materialize a full `.ovpn` config for any (provider, remote, proto)
//! combination.
//!
//! Materialized configs are written as deterministic real files under
//! [`paths::builtin_dir`], so they flow through the existing path-based scan,
//! recent, export and connect machinery unchanged. A path is recognized as
//! built-in when it lives under [`paths::builtin_dir`].

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::paths;

/// OpenVPN transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Proto {
    Udp,
    Tcp,
}

impl Proto {
    /// The wire name used in the config's `proto` line.
    pub fn as_str(self) -> &'static str {
        match self {
            Proto::Udp => "udp",
            Proto::Tcp => "tcp",
        }
    }

    /// Parse a wire name into a [`Proto`].
    pub fn from_name(name: &str) -> Option<Proto> {
        match name {
            "udp" => Some(Proto::Udp),
            "tcp" => Some(Proto::Tcp),
            _ => None,
        }
    }
}

impl fmt::Display for Proto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A built-in VPN provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    VpnGate,
}

impl Provider {
    /// Provider identifier used in directory names and export names.
    pub fn name(self) -> &'static str {
        match self {
            Provider::VpnGate => "vpn-gate",
        }
    }

    /// Identifier for the shared CA/cert/key template.
    pub fn key_id(self) -> &'static str {
        match self {
            Provider::VpnGate => "vpn-gate-key",
        }
    }

    /// Parse a provider name into a [`Provider`].
    pub fn from_name(name: &str) -> Option<Provider> {
        match name {
            "vpn-gate" => Some(Provider::VpnGate),
            _ => None,
        }
    }
}

/// A single materializable built-in config: one provider, one remote, one proto.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuiltinConfig {
    pub provider: Provider,
    /// The full `remote <host> <port>` line.
    pub remote: String,
    pub proto: Proto,
}

impl BuiltinConfig {
    /// Reconstruct the identity from a materialized path
    /// (`<builtin_dir>/<provider>/<proto>/<host>-<port>.ovpn`). The path layout
    /// is owned here — this is the single place that parses it back out.
    pub fn from_path(path: &Path) -> Option<Self> {
        if !is_builtin_path(path) {
            return None;
        }
        let stem = path.file_stem()?.to_str()?;
        let (host, port) = stem.rsplit_once('-')?;
        if host.is_empty() || port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let proto_dir = path.parent()?;
        let proto = Proto::from_name(proto_dir.file_name()?.to_str()?)?;
        let provider_dir = proto_dir.parent()?;
        let provider = Provider::from_name(provider_dir.file_name()?.to_str()?)?;
        Some(BuiltinConfig {
            provider,
            remote: format!("remote {host} {port}"),
            proto,
        })
    }

    /// The `host` and `port` of this config's remote.
    pub fn host_port(&self) -> Option<(&str, &str)> {
        remote_host_port(&self.remote)
    }
}

/// Header template with `{proto}` and `{remote}` placeholders, substituted by
/// [`build_config`].
const HEADER_TEMPLATE: &str = "\
dev tun
proto {proto}
{remote}
data-ciphers AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305:AES-128-CBC
auth SHA1
resolv-retry infinite
nobind
persist-key
persist-tun
client
verb 3";

/// Shared CA/cert/key block (`vpn-gate-key`) appended after the header.
const KEY_TEMPLATE: &str = "\
<ca>
-----BEGIN CERTIFICATE-----
MIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw
TzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh
cmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4
WhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu
ZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY
MTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc
h77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+
0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U
A5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW
T8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH
B5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC
B5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv
KBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn
OlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn
jh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw
qHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI
rU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV
HRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq
hkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL
ubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ
3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK
NFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5
ORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur
TkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC
jNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc
oyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq
4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA
mRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d
emyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=
-----END CERTIFICATE-----
</ca>

<cert>
-----BEGIN CERTIFICATE-----
MIICxjCCAa4CAQAwDQYJKoZIhvcNAQEFBQAwKTEaMBgGA1UEAxMRVlBOR2F0ZUNs
aWVudENlcnQxCzAJBgNVBAYTAkpQMB4XDTEzMDIxMTAzNDk0OVoXDTM3MDExOTAz
MTQwN1owKTEaMBgGA1UEAxMRVlBOR2F0ZUNsaWVudENlcnQxCzAJBgNVBAYTAkpQ
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA5h2lgQQYUjwoKYJbzVZA
5VcIGd5otPc/qZRMt0KItCFA0s9RwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD
4W8GmJe8zapJnLsD39OSMRCzZJnczW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQ
CjntLIWk5OLLVkFt9/tScc1GDtci55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67
XCKJnGB5nlQ+HsMYPV/O49Ld91ZN/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6h
p/0yXnTB//mWutBGpdUlIbwiITbAmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGD
ywIDAQABMA0GCSqGSIb3DQEBBQUAA4IBAQChO5hgcw/4oWfoEFLu9kBa1B//kxH8
hQkChVNn8BRC7Y0URQitPl3DKEed9URBDdg2KOAz77bb6ENPiliD+a38UJHIRMqe
UBHhllOHIzvDhHFbaovALBQceeBzdkQxsKQESKmQmR832950UCovoyRB61UyAV7h
+mZhYPGRKXKSJI6s0Egg/Cri+Cwk4bjJfrb5hVse11yh4D9MHhwSfCOH+0z4hPUT
Fku7dGavURO5SVxMn/sL6En5D+oSeXkadHpDs+Airym2YHh15h0+jPSOoR6yiVp/
6zZeZkrN43kuS73KpKDFjfFPh8t4r1gOIjttkNcQqBccusnplQ7HJpsk
-----END CERTIFICATE-----
</cert>

<key>
-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA5h2lgQQYUjwoKYJbzVZA5VcIGd5otPc/qZRMt0KItCFA0s9R
wReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD4W8GmJe8zapJnLsD39OSMRCzZJnc
zW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQCjntLIWk5OLLVkFt9/tScc1GDtci
55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67XCKJnGB5nlQ+HsMYPV/O49Ld91ZN
/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6hp/0yXnTB//mWutBGpdUlIbwiITbA
mrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGDywIDAQABAoIBAERV7X5AvxA8uRiK
k8SIpsD0dX1pJOMIwakUVyvc4EfN0DhKRNb4rYoSiEGTLyzLpyBc/A28Dlkm5eOY
fjzXfYkGtYi/Ftxkg3O9vcrMQ4+6i+uGHaIL2rL+s4MrfO8v1xv6+Wky33EEGCou
QiwVGRFQXnRoQ62NBCFbUNLhmXwdj1akZzLU4p5R4zA3QhdxwEIatVLt0+7owLQ3
lP8sfXhppPOXjTqMD4QkYwzPAa8/zF7acn4kryrUP7Q6PAfd0zEVqNy9ZCZ9ffho
zXedFj486IFoc5gnTp2N6jsnVj4LCGIhlVHlYGozKKFqJcQVGsHCqq1oz2zjW6LS
oRYIHgECgYEA8zZrkCwNYSXJuODJ3m/hOLVxcxgJuwXoiErWd0E42vPanjjVMhnt
KY5l8qGMJ6FhK9LYx2qCrf/E0XtUAZ2wVq3ORTyGnsMWre9tLYs55X+ZN10Tc75z
4hacbU0hqKN1HiDmsMRY3/2NaZHoy7MKnwJJBaG48l9CCTlVwMHocIECgYEA8jby
dGjxTH+6XHWNizb5SRbZxAnyEeJeRwTMh0gGzwGPpH/sZYGzyu0SySXWCnZh3Rgq
5uLlNxtrXrljZlyi2nQdQgsq2YrWUs0+zgU+22uQsZpSAftmhVrtvet6MjVjbByY
DADciEVUdJYIXk+qnFUJyeroLIkTj7WYKZ6RjksCgYBoCFIwRDeg42oK89RFmnOr
LymNAq4+2oMhsWlVb4ejWIWeAk9nc+GXUfrXszRhS01mUnU5r5ygUvRcarV/T3U7
TnMZ+I7Y4DgWRIDd51znhxIBtYV5j/C/t85HjqOkH+8b6RTkbchaX3mau7fpUfds
Fq0nhIq42fhEO8srfYYwgQKBgQCyhi1N/8taRwpk+3/IDEzQwjbfdzUkWWSDk9Xs
H/pkuRHWfTMP3flWqEYgW/LW40peW2HDq5imdV8+AgZxe/XMbaji9Lgwf1RY005n
KxaZQz7yqHupWlLGF68DPHxkZVVSagDnV/sztWX6SFsCqFVnxIXifXGC4cW5Nm9g
va8q4QKBgQCEhLVeUfdwKvkZ94g/GFz731Z2hrdVhgMZaU/u6t0V95+YezPNCQZB
wmE9Mmlbq1emDeROivjCfoGhR3kZXW1pTKlLh6ZMUQUOpptdXva8XxfoqQwa3enA
M7muBbF0XN7VO80iJPv+PmIZdEIAkpwKfi201YB+BafCIuGxIF50Vg==
-----END RSA PRIVATE KEY-----
</key>";

/// Deduplicated and sorted `remote <host> <port>` lines for a provider.
pub fn remotes(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::VpnGate => VPN_GATE_REMOTES,
    }
}

/// One [`BuiltinConfig`] per unique remote line for the provider, with the
/// given proto. The list is deduplicated and sorted by construction.
pub fn enumerate(provider: Provider, proto: Proto) -> Vec<BuiltinConfig> {
    remotes(provider)
        .iter()
        .map(|remote| BuiltinConfig {
            provider,
            remote: (*remote).to_string(),
            proto,
        })
        .collect()
}

/// Full `.ovpn` text: header (with proto + remote substituted) plus a blank
/// line plus the key template.
pub fn build_config(cfg: &BuiltinConfig) -> String {
    let header = HEADER_TEMPLATE
        .replace("{proto}", cfg.proto.as_str())
        .replace("{remote}", &cfg.remote);
    format!("{header}\n\n{KEY_TEMPLATE}")
}

/// Parse a `remote <host> <port>` line into `(host, port)`.
pub fn remote_host_port(remote: &str) -> Option<(&str, &str)> {
    let mut parts = remote.split_whitespace();
    let keyword = parts.next()?;
    let host = parts.next()?;
    let port = parts.next()?;
    if parts.next().is_some() || keyword != "remote" || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((host, port))
}

/// Write the built config to `<dir>/<host>-<port>.ovpn`, creating the directory
/// if needed, and return the path.
pub fn materialize(cfg: &BuiltinConfig, dir: &Path) -> Result<PathBuf> {
    let (host, port) = remote_host_port(&cfg.remote)
        .with_context(|| format!("invalid remote line: {}", cfg.remote))?;
    std::fs::create_dir_all(dir)
        .with_context(|| format!("cannot create directory {}", dir.display()))?;
    let path = dir.join(format!("{host}-{port}.ovpn"));
    std::fs::write(&path, build_config(cfg))
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

/// Whether a config path lives under the built-ins directory
/// ([`paths::builtin_dir`]).
pub fn is_builtin_path(path: &Path) -> bool {
    match paths::builtin_dir() {
        Ok(dir) => path.starts_with(&dir),
        Err(_) => false,
    }
}

/// The user-facing name for a built-in config path: `<host>-<port>` (its file
/// stem). `None` when the path is not a built-in. The display convention lives
/// here so recent/connect don't re-derive the layout.
pub fn display_name(path: &Path) -> Option<String> {
    if !is_builtin_path(path) {
        return None;
    }
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

/// For a built-in path, the export file name
/// `{provider}_{host}-{port}_{COUNTRY}.ovpn`, e.g.
/// `vpn-gate_public-vpn-38.opengw.net-1195_JP.ovpn`. Returns `None` if the path
/// is not a built-in or the remote can't be parsed.
pub fn export_name(path: &Path, country: &str) -> Option<String> {
    let cfg = BuiltinConfig::from_path(path)?;
    let (host, port) = cfg.host_port()?;
    Some(format!(
        "{}_{}-{}_{}.ovpn",
        cfg.provider.name(),
        host,
        port,
        country
    ))
}

/// VPN Gate remotes (deduplicated, sorted).
mod remotes;
use remotes::VPN_GATE_REMOTES;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

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

/// For a built-in path, the export file name
/// `{provider}_{host}-{port}_{COUNTRY}.ovpn`, e.g.
/// `vpn-gate_public-vpn-38.opengw.net-1195_JP.ovpn`. Returns `None` if the path
/// is not a built-in or the remote can't be parsed.
pub fn export_name(path: &Path, country: &str) -> Option<String> {
    if !is_builtin_path(path) {
        return None;
    }
    // Layout: <builtin_dir>/<provider>/<proto>/<host>-<port>.ovpn
    let provider_name = path.parent()?.parent()?.file_name()?.to_str()?;
    let provider = Provider::from_name(provider_name)?;
    let stem = path.file_stem()?.to_str()?;
    let dash = stem.rfind('-')?;
    let host = &stem[..dash];
    let port = &stem[dash + 1..];
    if host.is_empty() || port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}_{}-{}_{}.ovpn",
        provider.name(),
        host,
        port,
        country
    ))
}

/// VPN Gate remotes (deduplicated, sorted).
const VPN_GATE_REMOTES: &[&str] = &[
    "remote 2i6.opengw.net 443",
    "remote kaerunoheya.opengw.net 1970",
    "remote public-vpn-114.opengw.net 1195",
    "remote public-vpn-118.opengw.net 1195",
    "remote public-vpn-119.opengw.net 443",
    "remote public-vpn-120.opengw.net 443",
    "remote public-vpn-122.opengw.net 1195",
    "remote public-vpn-130.opengw.net 443",
    "remote public-vpn-131.opengw.net 443",
    "remote public-vpn-138.opengw.net 443",
    "remote public-vpn-146.opengw.net 443",
    "remote public-vpn-148.opengw.net 443",
    "remote public-vpn-150.opengw.net 443",
    "remote public-vpn-151.opengw.net 443",
    "remote public-vpn-153.opengw.net 443",
    "remote public-vpn-160.opengw.net 443",
    "remote public-vpn-165.opengw.net 443",
    "remote public-vpn-185.opengw.net 443",
    "remote public-vpn-204.opengw.net 443",
    "remote public-vpn-207.opengw.net 443",
    "remote public-vpn-232.opengw.net 443",
    "remote public-vpn-239.opengw.net 443",
    "remote public-vpn-240.opengw.net 443",
    "remote public-vpn-244.opengw.net 443",
    "remote public-vpn-249.opengw.net 443",
    "remote public-vpn-251.opengw.net 1195",
    "remote public-vpn-255.opengw.net 443",
    "remote public-vpn-257.opengw.net 443",
    "remote public-vpn-258.opengw.net 1195",
    "remote public-vpn-261.opengw.net 443",
    "remote public-vpn-37.opengw.net 443",
    "remote public-vpn-38.opengw.net 1195",
    "remote public-vpn-39.opengw.net 443",
    "remote public-vpn-40.opengw.net 443",
    "remote public-vpn-45.opengw.net 1195",
    "remote public-vpn-55.opengw.net 443",
    "remote public-vpn-78.opengw.net 1195",
    "remote public-vpn-94.opengw.net 1195",
    "remote public-vpn-97.opengw.net 443",
    "remote vpn100383739.opengw.net 1195",
    "remote vpn100951413.opengw.net 1121",
    "remote vpn101028669.opengw.net 1405",
    "remote vpn102450323.opengw.net 1745",
    "remote vpn107036699.opengw.net 22898",
    "remote vpn107712824.opengw.net 1810",
    "remote vpn111698412.opengw.net 1743",
    "remote vpn113680859.opengw.net 1696",
    "remote vpn116000962.opengw.net 47363",
    "remote vpn126573544.opengw.net 1726",
    "remote vpn131366305.opengw.net 1237",
    "remote vpn147580804.opengw.net 1602",
    "remote vpn150365618.opengw.net 1727",
    "remote vpn154698520.opengw.net 1491",
    "remote vpn165711755.opengw.net 1473",
    "remote vpn168643302.opengw.net 1195",
    "remote vpn183580192.opengw.net 1324",
    "remote vpn187430919.opengw.net 1195",
    "remote vpn190140048.opengw.net 1453",
    "remote vpn194250677.opengw.net 1305",
    "remote vpn196223048.opengw.net 1346",
    "remote vpn196751518.opengw.net 1454",
    "remote vpn203489968.opengw.net 1947",
    "remote vpn204889896.opengw.net 1428",
    "remote vpn212140070.opengw.net 1270",
    "remote vpn213799327.opengw.net 1581",
    "remote vpn213976710.opengw.net 1195",
    "remote vpn226748791.opengw.net 5639",
    "remote vpn229176608.opengw.net 1291",
    "remote vpn234471459.opengw.net 1412",
    "remote vpn243912077.opengw.net 1660",
    "remote vpn243982902.opengw.net 1969",
    "remote vpn248771747.opengw.net 1816",
    "remote vpn251634266.opengw.net 1791",
    "remote vpn25252525.opengw.net 1194",
    "remote vpn252787895.opengw.net 1938",
    "remote vpn254736373.opengw.net 1241",
    "remote vpn255806013.opengw.net 1953",
    "remote vpn262998764.opengw.net 1710",
    "remote vpn267620663.opengw.net 1655",
    "remote vpn268648531.opengw.net 1822",
    "remote vpn283661088.opengw.net 1951",
    "remote vpn284117207.opengw.net 1655",
    "remote vpn287798256.opengw.net 1195",
    "remote vpn291007099.opengw.net 1462",
    "remote vpn291876960.opengw.net 1217",
    "remote vpn295649515.opengw.net 1669",
    "remote vpn296034553.opengw.net 1937",
    "remote vpn296169938.opengw.net 1354",
    "remote vpn298204670.opengw.net 1195",
    "remote vpn300135762.opengw.net 1897",
    "remote vpn309255848.opengw.net 1195",
    "remote vpn309258753.opengw.net 1504",
    "remote vpn311904526.opengw.net 1561",
    "remote vpn315092128.opengw.net 1208",
    "remote vpn317018586.opengw.net 1195",
    "remote vpn326873107.opengw.net 1433",
    "remote vpn334153435.opengw.net 1642",
    "remote vpn344251639.opengw.net 1914",
    "remote vpn344424779.opengw.net 1522",
    "remote vpn350705742.opengw.net 1214",
    "remote vpn352150153.opengw.net 1641",
    "remote vpn354153193.opengw.net 1712",
    "remote vpn354871403.opengw.net 1550",
    "remote vpn355090085.opengw.net 1467",
    "remote vpn356087746.opengw.net 1461",
    "remote vpn356630442.opengw.net 7766",
    "remote vpn359681224.opengw.net 1844",
    "remote vpn360874921.opengw.net 1429",
    "remote vpn390160974.opengw.net 1195",
    "remote vpn392576910.opengw.net 1530",
    "remote vpn398338983.opengw.net 1195",
    "remote vpn400627339.opengw.net 1195",
    "remote vpn401574325.opengw.net 1890",
    "remote vpn403424469.opengw.net 1195",
    "remote vpn404025321.opengw.net 1800",
    "remote vpn408435278.opengw.net 1266",
    "remote vpn409975843.opengw.net 1540",
    "remote vpn410854768.opengw.net 1640",
    "remote vpn419493823.opengw.net 1195",
    "remote vpn422830275.opengw.net 1552",
    "remote vpn423975928.opengw.net 1931",
    "remote vpn439995456.opengw.net 1195",
    "remote vpn440186802.opengw.net 1851",
    "remote vpn440254653.opengw.net 1265",
    "remote vpn449254052.opengw.net 1686",
    "remote vpn451832275.opengw.net 1479",
    "remote vpn460244071.opengw.net 1966",
    "remote vpn461425710.opengw.net 1509",
    "remote vpn464496826.opengw.net 1894",
    "remote vpn464598114.opengw.net 1195",
    "remote vpn473031436.opengw.net 1711",
    "remote vpn475780323.opengw.net 1619",
    "remote vpn477916218.opengw.net 1810",
    "remote vpn484448685.opengw.net 1541",
    "remote vpn486259372.opengw.net 1669",
    "remote vpn487233765.opengw.net 1844",
    "remote vpn493078960.opengw.net 1896",
    "remote vpn496454283.opengw.net 1489",
    "remote vpn497910382.opengw.net 1945",
    "remote vpn500281930.opengw.net 1251",
    "remote vpn501589440.opengw.net 1371",
    "remote vpn509420157.opengw.net 1452",
    "remote vpn510493195.opengw.net 1195",
    "remote vpn511798507.opengw.net 1307",
    "remote vpn520507318.opengw.net 1626",
    "remote vpn521065574.opengw.net 1195",
    "remote vpn527493944.opengw.net 1996",
    "remote vpn536414965.opengw.net 1330",
    "remote vpn537593936.opengw.net 1919",
    "remote vpn540913720.opengw.net 1541",
    "remote vpn543104249.opengw.net 3359",
    "remote vpn544035541.opengw.net 1539",
    "remote vpn545920403.opengw.net 1195",
    "remote vpn546728922.opengw.net 1457",
    "remote vpn551216849.opengw.net 1416",
    "remote vpn551394294.opengw.net 1274",
    "remote vpn559183468.opengw.net 1363",
    "remote vpn564421139.opengw.net 1407",
    "remote vpn566557581.opengw.net 1690",
    "remote vpn569396308.opengw.net 1661",
    "remote vpn572894410.opengw.net 1758",
    "remote vpn574485521.opengw.net 1899",
    "remote vpn584376168.opengw.net 1195",
    "remote vpn589409974.opengw.net 1195",
    "remote vpn590418613.opengw.net 1195",
    "remote vpn590600903.opengw.net 1547",
    "remote vpn593168638.opengw.net 1947",
    "remote vpn595298283.opengw.net 1195",
    "remote vpn600557931.opengw.net 1195",
    "remote vpn610596269.opengw.net 1195",
    "remote vpn617491321.opengw.net 1227",
    "remote vpn620563931.opengw.net 1885",
    "remote vpn627963363.opengw.net 1195",
    "remote vpn638037739.opengw.net 1755",
    "remote vpn638506000.opengw.net 1814",
    "remote vpn646652894.opengw.net 1398",
    "remote vpn647586677.opengw.net 1387",
    "remote vpn661005850.opengw.net 1312",
    "remote vpn662919681.opengw.net 1195",
    "remote vpn666529495.opengw.net 1985",
    "remote vpn667439001.opengw.net 1436",
    "remote vpn668966783.opengw.net 1316",
    "remote vpn669212990.opengw.net 1891",
    "remote vpn671589964.opengw.net 1375",
    "remote vpn674894398.opengw.net 443",
    "remote vpn680207352.opengw.net 1195",
    "remote vpn682635912.opengw.net 1249",
    "remote vpn682939459.opengw.net 1603",
    "remote vpn683015078.opengw.net 1474",
    "remote vpn687149804.opengw.net 1636",
    "remote vpn687553365.opengw.net 1423",
    "remote vpn690662029.opengw.net 1612",
    "remote vpn699845531.opengw.net 1274",
    "remote vpn710361895.opengw.net 1855",
    "remote vpn731341284.opengw.net 1325",
    "remote vpn733188523.opengw.net 1195",
    "remote vpn734359090.opengw.net 1195",
    "remote vpn734894727.opengw.net 1449",
    "remote vpn735805327.opengw.net 1195",
    "remote vpn738429006.opengw.net 1195",
    "remote vpn739407957.opengw.net 1681",
    "remote vpn743727631.opengw.net 1522",
    "remote vpn744032933.opengw.net 1195",
    "remote vpn748512588.opengw.net 1879",
    "remote vpn752896674.opengw.net 1195",
    "remote vpn757481522.opengw.net 1916",
    "remote vpn759430340.opengw.net 1441",
    "remote vpn761139667.opengw.net 1286",
    "remote vpn764242710.opengw.net 1195",
    "remote vpn770844822.opengw.net 1330",
    "remote vpn773597226.opengw.net 1551",
    "remote vpn780913843.opengw.net 1410",
    "remote vpn783449766.opengw.net 5294",
    "remote vpn784219240.opengw.net 1468",
    "remote vpn784366499.opengw.net 1748",
    "remote vpn789962829.opengw.net 1195",
    "remote vpn796499400.opengw.net 1264",
    "remote vpn801674358.opengw.net 1328",
    "remote vpn805207565.opengw.net 1669",
    "remote vpn810318235.opengw.net 1284",
    "remote vpn818555213.opengw.net 1205",
    "remote vpn821248429.opengw.net 1195",
    "remote vpn822253469.opengw.net 9317",
    "remote vpn829352523.opengw.net 1392",
    "remote vpn829921213.opengw.net 1300",
    "remote vpn839458453.opengw.net 1828",
    "remote vpn846034044.opengw.net 1227",
    "remote vpn851717986.opengw.net 1229",
    "remote vpn852777788.opengw.net 1954",
    "remote vpn859486626.opengw.net 1195",
    "remote vpn860075787.opengw.net 1564",
    "remote vpn863519683.opengw.net 1195",
    "remote vpn864131273.opengw.net 1195",
    "remote vpn868858038.opengw.net 1904",
    "remote vpn877296508.opengw.net 1339",
    "remote vpn878075889.opengw.net 1833",
    "remote vpn879638290.opengw.net 1451",
    "remote vpn880426302.opengw.net 18896",
    "remote vpn885031424.opengw.net 26885",
    "remote vpn886841989.opengw.net 45218",
    "remote vpn893451313.opengw.net 1492",
    "remote vpn893657782.opengw.net 1195",
    "remote vpn894234214.opengw.net 1194",
    "remote vpn898575276.opengw.net 1488",
    "remote vpn907026687.opengw.net 1195",
    "remote vpn908794526.opengw.net 1973",
    "remote vpn909393181.opengw.net 1195",
    "remote vpn909782197.opengw.net 1195",
    "remote vpn914075597.opengw.net 1910",
    "remote vpn917620686.opengw.net 9391",
    "remote vpn920272004.opengw.net 1484",
    "remote vpn922726296.opengw.net 1195",
    "remote vpn925710881.opengw.net 1195",
    "remote vpn926729611.opengw.net 1268",
    "remote vpn934318830.opengw.net 1195",
    "remote vpn935303791.opengw.net 1641",
    "remote vpn939500782.opengw.net 1195",
    "remote vpn939813472.opengw.net 32835",
    "remote vpn941858045.opengw.net 1661",
    "remote vpn941915353.opengw.net 1664",
    "remote vpn944952031.opengw.net 1557",
    "remote vpn945009595.opengw.net 1687",
    "remote vpn951194492.opengw.net 1195",
    "remote vpn952483339.opengw.net 1408",
    "remote vpn954297571.opengw.net 1981",
    "remote vpn965061218.opengw.net 1195",
    "remote vpn966869813.opengw.net 1400",
    "remote vpn968224879.opengw.net 1415",
    "remote vpn975065048.opengw.net 1195",
    "remote vpn977560913.opengw.net 1761",
    "remote vpn977978908.opengw.net 1195",
    "remote vpn978549319.opengw.net 1991",
    "remote vpn981504377.opengw.net 1875",
    "remote vpn991137808.opengw.net 1468",
    "remote vpn991682963.opengw.net 1995",
    "remote vpn997268872.opengw.net 1585",
    "remote vpn999632787.opengw.net 1674",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn proto_display_and_parse() {
        assert_eq!(Proto::Udp.as_str(), "udp");
        assert_eq!(Proto::Tcp.as_str(), "tcp");
        assert_eq!(Proto::Udp.to_string(), "udp");
        assert_eq!(Proto::Tcp.to_string(), "tcp");
        assert_eq!(Proto::from_name("udp"), Some(Proto::Udp));
        assert_eq!(Proto::from_name("tcp"), Some(Proto::Tcp));
        assert_eq!(Proto::from_name("quic"), None);
    }

    #[test]
    fn provider_name_and_parse() {
        assert_eq!(Provider::VpnGate.name(), "vpn-gate");
        assert_eq!(Provider::VpnGate.key_id(), "vpn-gate-key");
        assert_eq!(Provider::from_name("vpn-gate"), Some(Provider::VpnGate));
        assert_eq!(Provider::from_name("nordvpn"), None);
    }

    #[test]
    fn build_config_substitutes_proto_remote_and_key() {
        let cfg = BuiltinConfig {
            provider: Provider::VpnGate,
            remote: "remote public-vpn-38.opengw.net 1195".to_string(),
            proto: Proto::Tcp,
        };
        let config = build_config(&cfg);
        assert!(config.starts_with("dev tun\n"));
        assert!(config.contains("\nproto tcp\n"));
        assert!(config.contains("\nremote public-vpn-38.opengw.net 1195\n"));
        assert!(config.contains("<ca>"));
        assert!(config.contains("-----BEGIN CERTIFICATE-----"));
        assert!(config.contains("<key>"));
        assert!(config.contains("-----BEGIN RSA PRIVATE KEY-----"));
        // blank line between header and key template
        assert!(config.contains("verb 3\n\n<ca>"));
    }

    #[test]
    fn build_config_uses_udp_proto() {
        let cfg = BuiltinConfig {
            provider: Provider::VpnGate,
            remote: "remote host.example 443".to_string(),
            proto: Proto::Udp,
        };
        assert!(build_config(&cfg).contains("\nproto udp\n"));
    }

    #[test]
    fn remotes_sorted_and_deduplicated() {
        let remotes = remotes(Provider::VpnGate);
        let mut sorted = remotes.to_vec();
        sorted.sort_unstable();
        assert_eq!(remotes, sorted);
        let unique: HashSet<_> = remotes.iter().collect();
        assert_eq!(unique.len(), remotes.len());
        for line in remotes {
            assert!(remote_host_port(line).is_some());
        }
        // The expected count for the vpn-gate provider.
        assert_eq!(remotes.len(), 277);
    }

    #[test]
    fn enumerate_produces_unique_configs() {
        let configs = enumerate(Provider::VpnGate, Proto::Udp);
        let mut seen = HashSet::new();
        for cfg in &configs {
            assert!(seen.insert(cfg.remote.clone()));
            assert_eq!(cfg.provider, Provider::VpnGate);
            assert_eq!(cfg.proto, Proto::Udp);
        }
        assert_eq!(configs.len(), 277);
        // The tcp enumeration covers the same remotes.
        let tcp = enumerate(Provider::VpnGate, Proto::Tcp);
        assert_eq!(tcp.len(), 277);
        assert!(tcp.iter().all(|cfg| cfg.proto == Proto::Tcp));
    }

    #[test]
    fn remote_host_port_parses_and_rejects() {
        assert_eq!(
            remote_host_port("remote host.example 443"),
            Some(("host.example", "443"))
        );
        assert_eq!(
            remote_host_port("remote public-vpn-38.opengw.net 1195"),
            Some(("public-vpn-38.opengw.net", "1195"))
        );
        assert_eq!(remote_host_port("host.example 443"), None);
        assert_eq!(remote_host_port("remote host.example"), None);
        assert_eq!(remote_host_port("remote host.example 443 extra"), None);
        assert_eq!(remote_host_port("remote host.example abc"), None);
        assert_eq!(remote_host_port(""), None);
    }

    #[test]
    fn materialize_writes_config_to_expected_path() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = BuiltinConfig {
            provider: Provider::VpnGate,
            remote: "remote public-vpn-38.opengw.net 1195".to_string(),
            proto: Proto::Udp,
        };
        let path = materialize(&cfg, dir.path()).unwrap();
        assert_eq!(path, dir.path().join("public-vpn-38.opengw.net-1195.ovpn"));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("dev tun"));
    }

    #[test]
    fn materialize_creates_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("does/not/exist/yet");
        let cfg = BuiltinConfig {
            provider: Provider::VpnGate,
            remote: "remote host.example 443".to_string(),
            proto: Proto::Tcp,
        };
        let path = materialize(&cfg, &nested).unwrap();
        assert!(path.parent().unwrap().is_dir());
        assert!(path.is_file());
    }

    #[test]
    fn is_builtin_path_matches_builtins_dir() {
        let dir = paths::builtin_dir().unwrap();
        let config_path = dir.join("vpn-gate/udp/public-vpn-38.opengw.net-1195.ovpn");
        assert!(is_builtin_path(&config_path));
        assert!(is_builtin_path(&dir));
        let config_dir = paths::config_dir().unwrap();
        assert!(!is_builtin_path(&config_dir.join("vmate.db")));
        assert!(!is_builtin_path(Path::new("/tmp/foo.ovpn")));
    }

    #[test]
    fn export_name_formats_builtin_path() {
        let dir = paths::builtin_dir().unwrap().join("vpn-gate").join("udp");
        let path = dir.join("public-vpn-38.opengw.net-1195.ovpn");
        assert_eq!(
            export_name(&path, "JP").unwrap(),
            "vpn-gate_public-vpn-38.opengw.net-1195_JP.ovpn"
        );
        // Non-built-in paths yield None.
        assert_eq!(export_name(Path::new("/tmp/foo.ovpn"), "JP"), None);
        // Unparseable remote (no numeric port) yields None.
        assert_eq!(export_name(&dir.join("just-a-host.ovpn"), "JP"), None);
    }
}

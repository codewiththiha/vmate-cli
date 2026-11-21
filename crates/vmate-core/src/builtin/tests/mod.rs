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

#[test]
fn from_path_reconstructs_identity() {
    let dir = paths::builtin_dir().unwrap().join("vpn-gate").join("tcp");
    let path = dir.join("public-vpn-38.opengw.net-1195.ovpn");
    let cfg = BuiltinConfig::from_path(&path).unwrap();
    assert_eq!(cfg.provider, Provider::VpnGate);
    assert_eq!(cfg.proto, Proto::Tcp);
    assert_eq!(cfg.host_port(), Some(("public-vpn-38.opengw.net", "1195")));
    assert_eq!(cfg.remote, "remote public-vpn-38.opengw.net 1195");
    // Not under the builtins dir -> None.
    assert!(BuiltinConfig::from_path(Path::new("/tmp/x-1.ovpn")).is_none());
}

#[test]
fn display_name_is_host_port_stem() {
    let dir = paths::builtin_dir().unwrap().join("vpn-gate").join("udp");
    let path = dir.join("public-vpn-38.opengw.net-1195.ovpn");
    assert_eq!(
        display_name(&path).unwrap(),
        "public-vpn-38.opengw.net-1195"
    );
    assert_eq!(display_name(Path::new("/tmp/foo.ovpn")), None);
}

mod storage;

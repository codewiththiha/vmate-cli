# vmate-cli v1.2.0

Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
arm64). Each zip contains the `vmate-cli` binary and `install.sh`, which copies
the binary onto your PATH.

## Built-in VPN configs

- `vmate-cli scan` with no directory scans the built-in vpn-gate remotes — the
  shared key is embedded once and each config is built on the fly, so there are
  no files to download. `--provider` selects the provider (default `vpn-gate`)
  and `--proto udp|tcp` picks the transport (default `udp`); re-scan with
  `--proto tcp` to try the other.
- Built-in configs appear in `recent` by their remote (`host-port`), cache
  their country like normal configs, and export as
  `provider_host-port_COUNTRY.ovpn`.

## Persistent defaults

- `vmate-cli scan --save-defaults --max 500 --timeout 20s` saves those as the
  defaults for future sessions; `vmate-cli connect --save-defaults
  --retry-count 5 --connect-timeout 10s --cooldown 60s --stability-grace 8s`
  does the same for the connect tunables. `--save-defaults` writes the new
  defaults and exits — it does not scan or connect.
- Each value resolves as `explicit flag → persisted setting → built-in
  default`, so a plain `vmate-cli scan` uses what you saved and a one-off
  `--max 200` still overrides for that run. Settings live in
  `vmate-cli/settings.json` inside your config directory and are
  human-editable.

## Reliability

- A CI gate now runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on
  every push, so the documented quality bar is enforced.
- Internal architecture pass: the retry/drop policy, built-in identity, and
  process-teardown logic were consolidated, and the process-registry isolation
  that had made the test suite flaky was fixed.

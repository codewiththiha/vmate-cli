# vmate-cli v1.1.0

Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
arm64). Each zip contains the `vmate-cli` binary and `install.sh`, which copies
the binary onto your PATH.

## Process safety

- The default cleanup now targets **only the OpenVPN processes vmate-cli
  spawned** (a per-process PID registry), instead of the global
  `killall -9 openvpn` sweep. Other VPN instances, containers, and
  system-managed OpenVPN processes are no longer touched.
- Processes are stopped gracefully: SIGTERM with a 3-second grace period,
  then SIGKILL only if still alive — so OpenVPN can clean up routes, TUN/TAP
  devices, and PID files before exiting.
- The global sweep is still available as an opt-in `--killall` flag (default
  off). **Breaking change:** the old `--no-killall` flag is removed; the
  default is per-process cleanup.

## Elevation

- New `--no-elevate` flag runs with current privileges instead of
  re-executing under sudo (mirrors the `VMATE_NO_ELEVATE` env var).

## Country detection and privacy

- vmate-cli now warns once when it falls back to the shared free ipinfo.io
  token, so it is clear that client IPs are sent to ipinfo.io. Set
  `--ipinfo-token` or `IPINFO_TOKEN` to use your own token.

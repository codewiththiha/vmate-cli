# vmate-cli v1.2.1

Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
arm64). Each zip contains the `vmate-cli` binary and `install.sh`, which copies
the binary onto your PATH.

## Linux: the database works again

- **Fixed: scan results disappeared on Linux.** `sudo` on Linux resets `HOME`
  to `/root`, so an elevated `vmate-cli scan` or `connect` stored its history in
  `/root/.config/vmate-cli/vmate.db` while `vmate-cli recent` — which never
  elevates — read `~/.config/vmate-cli/vmate.db` and reported nothing. macOS
  `sudo` keeps `HOME`, which is why the same build behaved differently per
  platform. vmate now re-applies your home, `XDG_*` and `VMATE_*` environment
  across the `sudo` re-exec and hands every root-created config directory,
  database, WAL sidecar and settings file back to your user.
- `vmate-cli doctor` reports the platform, your home, the resolved config
  directory and whether the database is really writable, and prints the
  `chown` that repairs a database left root-owned by an older release.
- The database is opened by path instead of through a `sqlite://` URL, so a
  database path containing `?` can no longer be parsed as query parameters.

## `scan` no longer asks for a password

- Scanning only probes configs — it never rewrites routes or interfaces — so it
  never blocks on a sudo prompt. When your sudo credentials are already cached
  the run is elevated for free; otherwise it runs with the privileges it has
  and hints at `sudo vmate-cli scan ...` only if every probe failed.
- `--save-defaults` is a pure write, so it never elevates on any command.

## Snappier switching and quitting

- `n` (next config) and Ctrl+C no longer wait out the 3s teardown grace period:
  a user-initiated switch escalates after 400ms, and shutdown cleanup polls for
  the processes to exit instead of sleeping a fixed second.
- Keys are polled every 50ms, so `n`, `r`, `c` and `q` land immediately and the
  uptime clock keeps ticking smoothly.
- A real SIGINT/SIGTERM — raw mode turns an interactive Ctrl+C into a key event
  — now kills only the OpenVPN processes vmate spawned and restores the
  terminal instead of leaving it in raw mode.

## Housekeeping

- CI runs on Linux *and* macOS, treats warnings as errors twice over (clippy
  with `-D warnings`, plus a workspace lints table that promotes `unused` and
  `unsafe_code` to errors) and fails on broken doc links.
- Result blocks are separated by a consistent blank line, so a scan report
  followed by an export summary no longer runs together.

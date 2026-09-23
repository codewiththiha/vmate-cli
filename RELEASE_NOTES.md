# vmate-cli v1.2.2

Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
arm64). Each zip contains the `vmate-cli` binary and `install.sh`, which copies
the binary onto your PATH.

## Linux: database fix retained & scan elevation restored

- **Linux database fix:** `sudo` on Linux resets `HOME` to `/root`, which caused
  elevated operations to write to `/root/.config/vmate-cli/vmate.db` instead of the
  invoking user's database. vmate carries your user environment (`HOME`, `USER`,
  `XDG_*`, `VMATE_*`) across the `sudo` re-exec and restores file ownership for
  any root-created database, WAL sidecar, config directory, or settings file, so
  `scan`, `connect`, and `recent` all share the exact same user history.
- **Root elevation restored for `scan`:** OpenVPN fundamentally requires root
  privileges (`CAP_NET_ADMIN`) to allocate the TUN device (`/dev/net/tun` on Linux
  or `utun` on macOS). An unprivileged `scan` is unable to test any configurations.
  `vmate-cli scan` now re-executes under `sudo` transparently as needed, while
  ensuring the resulting database belongs to your normal user account.
- **Diagnostics in `doctor`:** `vmate-cli doctor` checks platform, home, resolved
  config directory, and database write access, giving actionable repair commands
  if older runs left behind root-owned files.

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

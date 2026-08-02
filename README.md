# vmate-cli

A fast, interactive OpenVPN config **scanner**, **tester** and **connector** for
the command line. It finds `.ovpn` files on disk, tests them concurrently,
remembers which ones work, and lets you connect, switch and copy configs — all
from a keyboard-driven terminal UI backed by SQLite.

> **⚠️ Note:** vmate-cli intentionally runs `killall -9 openvpn` during
> connection switching and shutdown. This is a deliberate cleanup strategy, not
> a bug. Use `--no-killall` to disable the global sweep (per-process kills still
> happen).

---

## Features

- 🔍 **Scan** — recursively discover `.ovpn` files and test them concurrently.
- ⚡ **Connect** — intelligent retry, manual skip, deferred reshuffling.
- 🗂️ **Recent** — browse previously successful configs in a clickable TUI.
- 🎬 **All** — scan, store, then connect using only the filtered matches.
- 📦 **Export** — copy successful configs with sanitized, country-prefixed names.
- 🌍 **Filter** — filter by country code, case-insensitive, across every command.
- 🖱️ **Click-to-copy** — copy config paths from the recent TUI.
- 💾 **SQLite (WAL mode)** — persistent history with automatic migrations.
- 🩺 **Doctor** — environment and dependency checks.
- ⚙️ **Completions** — shell completions for bash/zsh/fish.
- 🔍 **Country detection** — filename heuristics, an IP cache, and a geo IP API.

## Requirements

- Rust **1.85+** (edition 2024)
- [OpenVPN](https://openvpn.net/) — `openvpn` on `PATH`, or pass `--openvpn-bin`
- Root/sudo for `scan`, `connect` and `all` (vmate-cli re-executes under `sudo`
  automatically on an interactive terminal; set `VMATE_NO_ELEVATE=1` to run
  without elevation — OpenVPN will likely fail)
- `killall` for the intentional global OpenVPN cleanup

## Build & Test

```bash
cargo build --release          # optimized, stripped binary → target/release/vmate-cli
cargo test                     # unit + integration tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Usage

```
vmate-cli <COMMAND> [OPTIONS]
```

Run `vmate-cli --help` for all options and `vmate-cli <COMMAND> --help` for
per-command help.

### Global options

These apply to every subcommand:

| Option | Description |
|---|---|
| `-f, --filter <COUNTRY>` | Filter by country code, e.g. `jp,kr`. Repeatable. |
| `--db <PATH>` | Path to the SQLite database (default `~/.config/vmate-cli/vmate.db`). |
| `--openvpn-bin <BIN>` | OpenVPN binary to use (default `openvpn`). |
| `--no-killall` | Disable the global `killall -9 openvpn` cleanup. |
| `--ipinfo-token <TOKEN>` | ipinfo.io API token (defaults to a bundled free token). |
| `-v, -vv, -q` | Verbosity / quiet logging. |
| `-h, --help` | Print help. |

### Commands

#### `scan` — discover, test and remember configs

```bash
# Scan ~/configs, keep testing until 20 Japan/Korea configs succeed.
# --max/-m controls concurrency, --timeout/-t is per-test seconds.
vmate-cli scan ~/configs --filter jp,kr --limit 20 --max 64 --timeout 15 -v

# Do not write results to the database
vmate-cli scan ~/configs --no-save

# Also copy this scan's filtered matches into ./out
vmate-cli scan ~/configs --filter jp --export ./out
```

`scan` tests every `.ovpn` file it finds, stores the successful ones in the
database (so they show up in `vmate-cli recent` later), and reports the configs
that match the current `--filter`.

#### `connect` — connect with intelligent retry

```bash
# Connect using stored JP candidates only
vmate-cli connect --filter jp

# Connect to an explicit config (fallbacks still respect the filter)
vmate-cli connect ./some.ovpn --filter jp

# Reject an explicit config that does not match the filter
vmate-cli connect ./us.ovpn --filter jp --strict-filter
```

`connect` picks candidates from the stored history, tries each one, retries a
failed handshake once, and drops a config from history after repeated failures.
Use the interactive keys below while connected.

#### `recent` — browse previously successful configs

```bash
# Show the last 50 successful configs in a TUI
vmate-cli recent

# Plain table output (no TUI)
vmate-cli recent --no-tui

# Show everything
vmate-cli recent --all

# Copy the newest config path immediately
vmate-cli recent --copy-first

# Also copy the listed configs into ./out
vmate-cli recent --filter jp --export ./out
```

In the TUI: press `Enter` or `c` to copy a config path, `/` to filter the list,
arrow keys / `j` `k` to move, and `q` / Ctrl+C to quit. Clicking a row copies
its path too.

#### `all` — scan, store, then connect

```bash
# Scan, then connect using only the filtered matches
vmate-cli all ~/configs --filter jp,kr

# Scan and report only (do not connect)
vmate-cli all ~/configs --no-connect

# Scan, connect, and also export this scan's matches
vmate-cli all ~/configs --filter jp --export ./out
```

`all` runs a full scan (storing successes as usual), reports the matches, then
hands them to the connect flow. `--no-connect` stops after the scan report.

#### `export` — copy stored successful configs

```bash
# Export JP configs to ./exported
vmate-cli export --filter jp --out ./exported
```

Exports configs from the database, naming each file `COUNTRY_<original name>`
and avoiding collisions with `_1`, `_2`, ... suffixes. For exporting from a
fresh scan or from the recent list, use `scan --export` or `recent --export`
instead.

#### `doctor` — environment checks

```bash
vmate-cli doctor
```

Checks for OpenVPN, root access, and database health.

#### `completions` — shell completion scripts

```bash
vmate-cli completions bash
vmate-cli completions zsh
vmate-cli completions fish
```

## Filtering

`--filter` is a global flag, case-insensitive, and can be repeated or comma
separated:

```bash
vmate-cli scan ./configs --filter JP,KR
vmate-cli scan ./configs --filter jp,kr
vmate-cli scan ./configs -f jp -f kr
```

`UNKNOWN` is an allowed value. An empty filter matches everything.

During a scan, the filter limits what is *reported* and *exported*, not what is
*tested* — unfiltered successes are still stored so they show up in
`vmate-cli recent` later.

## Country detection

Each config is tagged with a country using, in order:

1. **Filename heuristic** — a two-letter code embedded in the file name, e.g.
   `vpngate_20260801_jp_vpn-gate.ovpn` → `JP`. Fast, no network.
2. **IP cache** — the remote host is resolved and looked up in the SQLite cache.
3. **Geo IP API** — a lookup against ipinfo.io, persisted to the cache.

A free token is bundled, so country detection works with no configuration.
Override it with `--ipinfo-token` or the `IPINFO_TOKEN` environment variable.
Failures degrade to `UNKNOWN` — geo lookup never aborts a scan.

## Connect keys

While connected:

```
n       Next config (kill current, skip, defer)
r       Reconnect to the same config
c       Copy current config path
v       Toggle live OpenVPN output log
?       Show help
q       Quit
Ctrl+C  Quit and cleanup
```

`v` toggles a panel showing the OpenVPN process's output — the connection
handshake as well as live lines. `c` shows a `Copied: ...` confirmation that
fades after a few seconds.

Pressing `n` kills the current OpenVPN process group, runs `killall -9 openvpn`
(when enabled), marks the config as skipped (it is **not** deleted from history),
and moves it to the end of a shuffled deferred queue.

## Export

Two ways to copy configs out of vmate-cli:

- `vmate-cli scan <dir> --filter jp --export ./out` — copy **this scan's**
  fresh matches. The scan still stores its successes, so `vmate-cli recent` is
  updated as usual.
- `vmate-cli recent --filter jp --export ./out` — copy **previously scanned**
  stored configs matching the filter.
- `vmate-cli export --filter jp --out ./out` — copy stored configs to an output
  directory.

Exported files are named `COUNTRY_<sanitized-name>.ovpn`; collisions get `_1`,
`_2`, ... suffixes.

## Storage

The database lives at `~/.config/vmate-cli/vmate.db` by default (override with
`--db` or the `VMATE_DB` environment variable). WAL mode is enabled and
migrations run automatically on startup:

```bash
sqlite3 ~/.config/vmate-cli/vmate.db "PRAGMA journal_mode;"   # → wal
```

## Architecture

```
.
├── Cargo.toml               # workspace
├── migrations/              # SQLite schema
├── crates/
│   ├── vmate-core/          # domain logic (UI-agnostic)
│   │   ├── country.rs / filter.rs   # --filter parsing & matching
│   │   ├── db/              # SQLite pool, models, repository (WAL)
│   │   ├── ovpn/            # parser, cipher repair, process runner, monitor
│   │   ├── geo/             # country detection (filename/IP cache/geo API)
│   │   ├── scan/            # concurrent test orchestration
│   │   ├── connect/         # candidate queue + connect session
│   │   ├── export/          # sanitized config export
│   │   └── system/          # process killer, root, signals
│   └── vmate-cli/           # clap CLI, commands, TUIs, progress, clipboard
│       └── tests/           # integration tests (assert_cmd)
```

Design principles:

- **No global mutable state.** Everything is constructed per run and passed in.
- **Traits for external effects.** `VpnTester`, `OpenVpnRunner`,
  `ProcessKiller`, `GeoLocator` and `ConnectHost` keep the core testable.
- **RAII cleanup.** `CleanupGuard` and `TuiGuard` restore the terminal and kill
  stale OpenVPN processes even on panic/error paths.
- **Structured concurrency.** `JoinSet` + `Semaphore` + `CancellationToken` for
  scans; `tokio::select!` for the interactive connect loop.

## License

[MIT](LICENSE)

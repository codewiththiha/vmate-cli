# vmate-rs

A modern, idiomatic **Rust** port of the [Go `vmate-cli`](https://github.com/codewiththiha/vmate-cli)
OpenVPN config scanner, tester and connector.

> **⚠️ Note:** vmate intentionally runs `killall -9 openvpn` during connection
> switching and shutdown. This is not a bug — it is the tool's cleanup strategy,
> preserved from the original Go implementation. Use `--no-killall` to disable
> the global sweep (per-process kills still happen).

---

## Features

- 🔍 **Scan** a directory recursively for `.ovpn` files and test them concurrently.
- ⚡ **Connect** with intelligent retry, manual skip, and deferred reshuffling.
- 🗂️ **Recent** — browse previously successful configs (SQLite-backed).
- 🎬 **All** — scan, store, then connect using only the filtered matches.
- 📦 **Export** successful configs with sanitized, country-prefixed names.
- 🌍 **Filter** by country code, case-insensitive, across every command.
- 🩺 **Doctor** — environment/dependency checks.
- ⚙️ **Completions** for bash/zsh/fish.
- 💾 **SQLite (WAL mode)** storage with migrations.
- 🖱️ **Click-to-copy** recent entries in the TUI (keyboard copy always works).

## Requirements

- Rust **1.85+** (edition 2024)
- [OpenVPN](https://openvpn.net/) (`openvpn` on `PATH`, or `--openvpn-bin`)
- Root/sudo for `scan`, `connect`, and `all` (vmate re-executes under `sudo`
  automatically on an interactive terminal; set `VMATE_NO_ELEVATE=1` to run
  without elevation — OpenVPN will likely fail)
- `killall` for the intentional global OpenVPN cleanup

## Build & Test

```bash
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Usage

```bash
# Scan ~/configs, keep testing until 20 Japan/Korea configs succeed.
# --max / -m controls concurrency, --timeout / -t is per-test seconds
# (Go-compatible), and they combine with --filter and -v.
vmate scan ~/configs --filter jp,kr --limit 20 --max 64 --timeout 15 -v

# Connect using stored JP candidates only
vmate connect --filter jp

# Connect to an explicit config (fallbacks still respect the filter)
vmate connect ./some.ovpn --filter jp

# Show recent successful configs (TUI with click-to-copy)
vmate recent --filter jp,kr

# Scan then connect using only filtered matches
vmate all ~/configs --filter jp,kr

# Export JP configs to ./exported
vmate export --filter jp --out ./exported

# Check your environment
vmate doctor

# Generate shell completions
vmate completions bash
```

### Filtering

`--filter` is a global flag:

```bash
vmate scan ./configs --filter JP,KR
vmate scan ./configs --filter jp,kr
vmate scan ./configs -f jp -f kr
```

Values are case-insensitive; `UNKNOWN` is allowed. An empty filter matches
everything. During a scan, the filter limits what is *reported/exported*, not
what is *tested* — unfiltered successes are still stored in the database so
they show up in `vmate recent` later.

### Country lookup

Configs are tagged with a country using, in order: a two-letter code embedded
in the file name (e.g. `vpngate_..._jp_...ovpn`), the SQLite IP cache, then
the ipinfo.io API. The free ipinfo.io token from the original Go vmate-cli is
used by default, so no configuration is needed; override it with
`--ipinfo-token` or `IPINFO_TOKEN`.

### Connect keys

While connected:

```
n       Next config (kill current, skip, defer)
r       Reconnect to the same config
c       Copy current config path
v       Toggle verbose OpenVPN output
?       Show help
q       Quit
Ctrl+C  Quit and cleanup
```

Pressing `n` kills the current OpenVPN process group, runs `killall -9 openvpn`
(when enabled), marks the config as skipped (it is **not** deleted from
history), and moves it to the end of a shuffled deferred queue.

## Architecture

```
vmaters/
├── Cargo.toml               # workspace
├── migrations/              # SQLite schema
├── crates/
│   ├── vmate-core/          # domain logic (UI-agnostic)
│   │   └── src/
│   │       ├── country.rs / filter.rs   # --filter parsing & matching
│   │       ├── db/          # SQLite pool, models, repository (WAL)
│   │       ├── ovpn/        # parser, cipher repair, process runner, monitor
│   │       ├── geo/         # country detection (filename/IP cache/ipinfo)
│   │       ├── scan/        # concurrent test orchestration
│   │       ├── connect/     # candidate queue + connect session
│   │       ├── export/      # sanitized config export
│   │       └── system/      # process killer, root, signals
│   └── vmate-cli/           # clap CLI, commands, TUIs, progress, clipboard
│       └── tests/           # integration tests (assert_cmd)
└── tests/                   # (workspace-level integration tests live in vmate-cli/tests)
```

Design principles:

- **No global mutable state.** Everything is constructed per run and passed in.
- **Traits for external effects.** `VpnTester`, `OpenVpnRunner`,
  `ProcessKiller`, `GeoLocator` and `ConnectHost` make the core testable.
- **RAII cleanup.** `CleanupGuard` (and `TuiGuard`) restore the terminal and
  kill stale OpenVPN processes even on panic/error paths.
- **Structured concurrency.** `JoinSet` + `Semaphore` + `CancellationToken`
  for scans; `tokio::select!` for the interactive connect loop.

## Storage

The database lives at `~/.config/vmate-cli/vmate.db` by default (override with
`--db` or `VMATE_DB`). WAL mode is enabled automatically and migrations run on
startup:

```bash
sqlite3 ~/.config/vmate-cli/vmate.db "PRAGMA journal_mode;"   # → wal
```

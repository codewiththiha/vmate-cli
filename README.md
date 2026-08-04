# vmate-cli

A fast, interactive OpenVPN config **scanner**, **tester** and **connector** for
the command line. It finds `.ovpn` files on disk, tests them concurrently,
remembers which ones work, and lets you connect, switch and copy configs — all
from a keyboard-driven terminal UI backed by SQLite.

> **⚠️ Note:** by default vmate-cli cleans up only the OpenVPN processes it
> spawned: each process group is sent SIGTERM, given a grace period, then
> SIGKILLed if needed. Pass `--killall` to also run the `killall -9 openvpn`
> sweep (the behavior of the original Go tool) during connection switching and
> shutdown.

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
- `killall` (optional) only if you pass `--killall` for the global OpenVPN sweep

## Build & Test

```bash
cargo build --release          # optimized, stripped binary → target/release/vmate-cli
cargo test                     # unit + integration tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

## Install from a release

Releases are built automatically when a `v*` tag is pushed. Each release
attaches a zip per platform (`vmate-cli-<version>-<target>.zip`):

| Target | Binary | Notes |
| ------ | ------ | ----- |
| `aarch64-apple-darwin` | Apple Silicon Macs | arm64 |
| `x86_64-apple-darwin` | Intel Macs | x86_64 |
| `x86_64-unknown-linux-gnu` | Linux x86_64 | most servers / desktops |
| `aarch64-unknown-linux-gnu` | Linux arm64 | Pi, AWS Graviton, etc. |

Windows is not shipped — vmate-cli is Unix-only (process groups, root checks,
`killall` when `--killall` is used).

Download and extract a zip, then install the binary to your PATH:

```bash
tar -xzf vmate-cli-1.0.1-aarch64-apple-darwin.zip
cd vmate-cli-1.0.1-aarch64-apple-darwin
sudo ./install.sh        # copies vmate-cli → /usr/local/bin
vmate-cli --help
```

`install.sh` can also replace an existing install, install to a different
directory by editing the `DEST`/`OPERATION` variables at the top, and remove
itself with `sudo ./install.sh --uninstall`.

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
| `--killall` | Also run `killall -9 openvpn` on shutdown/switch. Default is per-process cleanup of only the openvpn processes vmate spawned. |
| `--no-elevate` | Don't re-execute under sudo; run with current privileges (OpenVPN will likely fail). |
| `--ipinfo-token <TOKEN>` | ipinfo.io API token (defaults to a bundled free token). |
| `--save-defaults` | Persist explicitly-passed default flags (e.g. `--max`, `--timeout`, `--retry-count`) to the config file for future sessions. |
| `-v, -vv, -q` | Verbosity / quiet logging. |
| `-h, --help` | Print help. |

### Persisted defaults

The scan/connect tunables (`--max`, `--limit`, `--timeout`, `--connect-timeout`,
`--cooldown`, `--retry-count`, `--stability-grace`) have built-in defaults. Pass
`--save-defaults` alongside the value flags to persist them for future sessions:

```bash
vmate-cli scan    --save-defaults --max 500 --timeout 20
vmate-cli connect --save-defaults --retry-count 5 --connect-timeout 10 --cooldown 60 --stability-grace 8
```

The `scan` and `all` commands persist their workers (`--max`), `--limit`, and
`--timeout` defaults; `connect`/`all` persist `--connect-timeout`, `--cooldown`,
`--retry-count`, and `--stability-grace`. The value flags also apply for that
run, and a plain command resolves to the persisted values. `--retry-count`
controls how many times a failing config is retried before it is dropped from
history; `--connect-timeout` is the handshake threshold, `--cooldown` the delay
before retrying a recently-failed config, and `--stability-grace` how long a
connected session must last before its crash resets the retry budget.

Only the flags you **explicitly pass** are saved; unmentioned tunables keep
their existing persisted or built-in default. Persisted settings live in
`~/.config/vmate-cli/settings.json` and can be edited by hand. Each tunable is
resolved as:

```
explicit CLI flag → persisted setting → built-in default
```

A missing or corrupt `settings.json` is ignored and falls back to the built-in
defaults.

### Commands

#### `scan` — discover, test and remember configs

```bash
# Scan ~/configs, keep testing until 20 Japan/Korea configs succeed.
# --max/-m controls concurrency, --timeout/-t is per-test seconds.
vmate-cli scan ~/configs --filter jp,kr --limit 20 --max 64 --timeout 15 -v

# Scan the built-in vpn-gate remotes over UDP (no directory argument)
vmate-cli scan --filter jp

# Try the same built-in remotes over TCP instead
vmate-cli scan --proto tcp

# Do not write results to the database
vmate-cli scan ~/configs --no-save

# Also copy this scan's filtered matches into ./out
vmate-cli scan ~/configs --filter jp --export ./out
```

`scan` tests every `.ovpn` file it finds, stores the successful ones in the
database (so they show up in `vmate-cli recent` later), and reports the configs
that match the current `--filter`.

With no directory argument, `scan` materializes the built-in configs for the
chosen provider and protocol into `~/.config/vmate-cli/builtin/<provider>/<proto>/`
and scans those. `--provider` selects the built-in provider (default `vpn-gate`)
and `--proto` selects the transport protocol (`udp` or `tcp`, default `udp`);
re-scan with `--proto tcp` to try the other protocol.

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

# Scan the built-in vpn-gate remotes and connect (no directory argument)
vmate-cli all --filter jp,kr

# Scan and report only (do not connect)
vmate-cli all ~/configs --no-connect

# Scan, connect, and also export this scan's matches
vmate-cli all ~/configs --filter jp --export ./out
```

`all` runs a full scan (storing successes as usual), reports the matches, then
hands them to the connect flow. `--no-connect` stops after the scan report. As
with `scan`, omitting the directory scans the built-in remotes for
`--provider`/`--proto`.

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

`vmate-cli` installs completion scripts for bash, zsh and fish into the
standard location for each shell, then prints how to activate them:

```bash
vmate-cli completions bash
vmate-cli completions zsh
vmate-cli completions fish
```

For zsh it prefers a Homebrew `zsh-completions` dir already on `$fpath`
(falling back to `~/.zfunc`). After installing, restart your shell — or just
run `compinit` in zsh to pick it up immediately.

If you need the raw script (for example to capture it into a dotfiles repo),
use `--print`:

```bash
vmate-cli completions zsh --print > _vmate-cli
```

Manual one-liners, if you prefer to place the script yourself:

```bash
# bash
echo 'source <(vmate-cli completions bash --print)' >> ~/.bashrc

# zsh (with fpath + compinit)
mkdir -p ~/.zfunc && vmate-cli completions zsh --print > ~/.zfunc/_vmate-cli
echo 'fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit' >> ~/.zshrc

# fish
mkdir -p ~/.config/fish/completions
vmate-cli completions fish --print > ~/.config/fish/completions/vmate-cli.fish
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

By default vmate sends client IPs to ipinfo.io under a shared free-tier token,
so country detection works with no configuration. That shared token is
rate-limited and shared across all vmate-cli users — for privacy-sensitive
setups, provide your own token with `--ipinfo-token` or the `IPINFO_TOKEN`
environment variable (a warning is emitted when the shared token is in use).
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
fades after a few seconds. Keys respond immediately, even while a connection
is being established or switched. When a config is removed after repeated
failures, a `removed <file> from recent list` notice is shown briefly, and the
Config line shows the `.ovpn` file name (not the full path).

Pressing `n` gracefully kills the current OpenVPN process group (SIGTERM, then
SIGKILL after the grace period), runs `killall -9 openvpn` too when `--killall`
is enabled, marks the config as skipped (it is **not** deleted from history),
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

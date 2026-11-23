# Scan memory, concurrency and input measurements

## Real 500-worker GitHub runner measurement

Run: https://github.com/codewiththiha/vmate-cli/actions/runs/44897720456
Source commit: 5f036ae. Linux 7.0.0-1017-azure, x86_64, four visible CPUs,
Ubuntu 24.04 container, OpenVPN 2.6.19. Container limit: 5 GiB, 2048 tasks.
The benchmark raises only its own descriptor limit to at least 8192.

Command (template materialized by the existing built-in scanner):

```sh
python3 tools/scan_bench.py --binary target/debug/vmate-cli \
  --template "$template" --output /evidence/workers --workers 64 500
```

Workload: one real OpenVPN child per worker, separate configs, built-in crypto
material, remote replaced with a local UDP sink that drains packets but does not
complete TLS. No public relay receives the 500-worker stress test. Each of q,
Ctrl+C and SIGTERM gets a separate full-occupancy run.

| Workers | Parent RSS | Summed child RSS | Sampled child PSS | Shutdown range |
|---|---:|---:|---:|---:|
| 64 | 20.5–20.8 MiB | 594.6–594.8 MiB | 94.7–94.8 MiB | 26–70 ms |
| 500 | 29.7–30.0 MiB | 4645.8–4649.7 MiB | 683.4–685.3 MiB | 157–179 ms |

All six runs reached the requested live-child count, exited 0, left no observed
child PID, and preserved routes, links, rules and /etc/resolv.conf hash.

RSS sums repeatedly count shared mappings and are not an actual physical-memory
bill. PSS is sampled after one second at full occupancy, despite the historical
JSON field name `peak_children_pss_sum_kib`; it is not an absolute peak. Parent
RSS and child PSS are different metrics and cannot be added as an exact total.
Socket/kernel buffers, cache accounting, actual TLS completion, authentication,
geo lookups and sustained public-relay load are not fully measured here.

Default concurrency remains 64. Approximately 700 MiB of child PSS alone is not a
safe universal default for small machines; 500 also multiplies CPU/network/DNS
pressure. Explicit `--max 500` remains available. Effective workers are capped by
`max(1, (RLIMIT_NOFILE - 128) / 6)` to reserve descriptor headroom. A soft limit
of 1024 allows 149 probes, not 500; use an appropriate user-authorized limit
(e.g. `ulimit -n 4096`) if the machine and its hard limit permit it.

## Before/after local comparison

Machine: Linux sandbox, 1982 MiB reported RAM, OpenVPN 2.6.14/OpenSSL 3.5.7,
same built-in template and local UDP sink. Before: 08f81c0. After: 5f036ae.
Both executables built in debug mode with the same installed toolchain.

```sh
python3 tools/scan_bench.py --binary "$binary" --template /home/user/probe.ovpn \
  --output "$results" --workers 64 --actions TERM
python3 tools/input_latency.py "$binary"
```

For 64 workers, before/after child PSS was 122402/122408 KiB and parent RSS
16916/16596 KiB. SIGTERM-to-exit was 52/45 ms in these single samples. This is
not evidence of a significant memory reduction; OpenVPN children dominate.
The changed input architecture addresses starvation, not TLS-process memory.

With a fake VPN continuously flooding logs, the old version did not exit on q
within the five-second watchdog. After the change, q and Ctrl+C exited in about
32 ms, and SIGTERM in about 16 ms. This includes teardown of a cooperative fake
process, not worst-case real-VPN route cleanup. The runner independently measured
similar flooded-output latencies. Raw transcript rendering is rate-limited to
10 Hz; input reading is independent of output, with a 20 ms terminal poll bound.

## Resource contracts and limits

- Output channels hold at most 512 lines per child; individual line bytes are
  not currently capped, so this is not an absolute byte limit.
- Input has one owned thread, a 32-command queue, and independent priority quit.
- Worker slots are held through teardown; cancellation joins workers together,
  rather than paying the grace interval serially for each worker.
- Connected sessions get up to three seconds for graceful OpenVPN cleanup.
  SIGKILL escalation has a further bounded reap wait and logs kernel stalls.
- Discovered path count and successful results are bounded by input size, not a
  global byte budget. Existing geo memoization is also input-dependent.
- SIGKILL, kernel uninterruptible sleep and machine failure cannot be guaranteed
  to clean up by application code. No all-edge-case or zero-latency claim is made.

A follow-up real 64-worker local SIGHUP test exited in 46 ms with no child PIDs
or network-state changes. Later CI includes hangup and mocked managed teardown;
keep its results distinct from the six-run measurement table above.

Latest input-owner rerun: 4f6ef20, run 44898514503. All eight 64/500-worker q,
Ctrl+C, SIGTERM and SIGHUP cases passed with unchanged network state and no child
PIDs. At 500 workers teardown took 196–223 ms. The raw-output flood and mocked
NetworkManager cases exited in 15–32 ms. Summary: docs/testing/scan-hangup-results.json.

Final teardown/error-classification run: 2a4c36e, run 44899043086. All eight cases
passed. At 500 workers, child PSS samples were 683.4–685.4 MiB and teardown took
175–193 ms. The persisted scan-hangup-results.json now identifies this final run.
This still measures a local stalled-handshake workload, not 500 full public TLS
sessions. No evidence justifies increasing the universal default to 500.

### Desktop branch verification (2026-11-21)

- Rust/Slint preview uses a separate engine crate; CLI remains in apps/cli.
- Local workspace suite: 162 tests; clippy all-target/all-feature warnings denied.
- Headless software rendering covers 15 scenarios at two sizes (30 PNGs).
- Xvfb/Openbox native Linux window identity/minimize/maximize/close passed.
- Linux amd64 debug system/bundled installation packages were validated locally.
- No measured startup/latency claim; polling is 100ms, not a latency benchmark.
- No native macOS/Wayland/polkit/egress or signed release validation locally.
- GitHub desktop matrix started; results must be inspected before acceptance.


### One-button Home revision

- Local suite: 170 tests; automatic coordinator uses a fake service backend in tests.
- Four capture passes culminated in 96 fixture PNGs at four sizes, minimum 480×600.
- Power glyph and banner-induced navigation overflow were found and corrected.
- Named presets independently configure manual Scan and automatic Connect.
- Rescan loops are limited to 1–20; retry pauses cap at 30 seconds.
- No new live VPN/egress or macOS automatic activation validation is claimed.


### CLI/desktop parity and branding

- 171 local workspace tests; real CLI/engine shared-database integration added.
- Actual Slint binding test drives a scan and receives 277 fixture results.
- Logo/wordmark centers align; pixel assertion permits at most two-pixel drift.
- Background history polling: configured two seconds, not a measured latency guarantee.
- VMATE_DB override now shared; CLI-only scan flags do not rewrite GUI presets.
- New hosted runs blocked by absent GitHub authentication (push dry-run rejected).
- Older run 45118973809: Linux review passed; package jobs failed. Logs inaccessible.


### Hosted runner verification on 532f3c2

- desktop-review run 45125829154 passed: Linux native/capture review and release
  packages on Linux amd64/arm64 and macOS Intel/Apple Silicon, both editions.
- live-vpngate run 45125829140 passed built-in public-relay checks.
- Earlier live run 45125260430 verified five raw CLI session actions, network
  restoration, engine reading four CLI relays, and 277 engine probes/32 successes.
- Authenticated logs identified Ubuntu /lib versus /usr/lib package provenance;
  bounded alias verification fixes redistribution notice collection.
- macOS shared-history comparison now uses canonical /private/var paths.
- macOS workspace run 45125829214 exposed a test click offset from hiding Linux
  chrome; the revised test exercises both content layouts locally.
- NetworkManager authorization/automatic tunnel egress remain unvalidated.

## Native frame and optional Linux tray correction

Local branch fix/desktop-input-tray removes the client MenuBar, custom Linux
chrome and drag callback. The window manager owns a single native title bar;
content contains no window-drag region. Normal taskbar identity is unchanged.

- Linux tray preference defaults to false and old JSON preferences still load.
- One bounded worker channel owns optional StatusNotifier registration off the
  GUI thread. Shutdown unregisters and awaits service completion.
- Closing hides only while a host is available; explicit Quit cancels owned work.
  Host loss restores the window, and an unavailable host cannot trap a hidden app.
- Real Xvfb/Openbox tests passed body-drag rejection, pointer navigation, text
  entry and persisted Save, native minimize/maximize and normal close/exit.
- Real app/D-Bus fixture tests passed default-off, close/reopen, hidden Quit,
  disable/unregister, host-loss restoration and unavailable-host close/exit.
- 172 workspace Rust tests and 12 Python package tests passed locally. Strict
  workspace clippy/all-targets/all-features, formatting and source guards passed.
- Notice/source generation retained 483 Cargo.lock-verified Linux dependencies,
  including the new Unlicense-licensed ksni tray implementation.
- Reviewed 96 component captures and an actual native-window Home capture. There
  is one system title bar, no extra menu strip, and compact bottom navigation
  fits at 480px. The fixture banner exists only in deterministic capture mode.
- GNOME/KDE tray rendering, Wayland and updated macOS interaction are not locally
  validated. The published desktop-v1.2.4-alpha.1 assets are not changed here.

## Shared palette, aligned settings and relay countries

Branch fix/desktop-theme-countries corrects the mixed custom-light/standard-dark
palette that made inputs unreadable on dark desktops. Theme derives from the
standard widget palette; System/Light/Dark selection previews and persists.
SettingField supplies aligned labels, equal heights and visible control borders.
The Settings form has a bounded width and explicit non-stretching vertical flow.

- Reviewed actual light/dark Settings, bottom-of-scroll and narrow connected
  captures. Country labels, field values and checkbox labels are readable.
- Captures now cover 192 states: 24 scenarios, four widths and two palettes.
- Actual rendered preset-input pixels meet at least 4.5:1 contrast in both
  palettes at 480x600 and 1160x770 in the renderer regression test.
- Default lookup uses the existing IpInfoGeoLocator, shared token/cache and
  optional IPINFO_TOKEN override. Saved opt-outs remain preserved. Fixtures
  explicitly disable external lookup; ordinary tests do not send relay IPs.
- A real built-in public-relay engine probe tested 277, saved 35 successful
  relays and resolved countries for all 35. Country breakdown: JP20, KR9,
  HK1, ID1, NZ1, RU1, TH1, VN1. Export bytes verified; no tunnel tested.
- CountryCode-to-name coverage checks every ISO entry. Home shows the known
  connected relay country, never stale metadata after disconnect, and no
  inferred location for unknown relays/external profiles.
- Native X11 tests persist Dark through the real appearance selector and Save;
  text/pointer/native-frame and disposable-host tray lifecycle checks still pass.
- New ISO display dependency notices/source inventory includes 484 Linux crates.
  No updated release, remote push, GNOME/KDE or Wayland validation is claimed.
- Final local validation: 175 workspace Rust tests, 12 package tests, strict
  all-targets/all-features workspace clippy, rustfmt, shape/dups, changed-file
  comment/budget and diff guards passed. Final rebuilt binary regenerated all
  192 captures and passed native pointer/theme-save and tray lifecycle tests.

## Session completion and Linux activation

Branch fix/desktop-session-controls corrects stale Disconnecting presentation.
Home now requires a busy operation before showing that label, and completion
clears the cancellation message. Managed sessions observe NetworkManager UUID
state every second; established teardown can finish when NM confirms inactive
instead of waiting for a slow nmcli down client. Activation cancellation retains
command completion before accepting inactivity. Unconfirmed cleanup remains an
error; the app does not infer process ownership from unrelated OpenVPN PIDs.

- Linux abstract Unix sockets serialize per-user desktop startup before engine
  creation. A second launch signals the existing window; capture mode bypasses
  ownership. A bounded timer accepts requests and checks peer UID. Socket lifetime
  follows the event-loop timer, with no lockfile, detached worker or stale path.
- Unknown relay context menus offer Look up country now. This explicit action can
  request country metadata even if automatic lookup is disabled. It uses the
  existing resolver/cache and a ten-second cancellation-aware deadline; country
  updates do not change handshake success counters or timestamps.
- Existing Settings rescan-loop limit remains 1–20. Boundary tests verify saved
  JSON and exactly one/twenty rescan calls; no duplicate preference was added.
- Home replaces the linear bar with a rotating arc around the power button.
  Reduced motion freezes the arc; connected idle sessions do not show a spinner.
- Isolated nmcli fixture: cancellation 0.316s despite an eight-second down client;
  external disconnect 0.750s. Local Linux sandbox, debug build, commands:
  `python3 tools/managed_smoke.py target/debug/vmate-cli`. Not live NM/VPN proof.
- Native X11 pointer/theme-save/frame checks pass. Disposable D-Bus tray checks
  now also launch a second process while hidden, verify successful exit, one
  visible window and subsequent hide/reopen. No GNOME/KDE/Wayland claim.
- Regenerated 192 captures in /home/user/session-review; inspected narrow light
  Home: arc is outside the button, with no linear indicator or clipped label.
- Final local gates: 178 workspace Rust tests, 12 packaging tests, strict
  all-targets/all-features clippy, fmt, shape/dups, source comment/budget and
  diff guards pass. Existing YAML artifact globs are not source block comments.
  No push, release or main merge performed; live NM/polkit/egress remain untested.

## Usable settings, selected exports and persistent tray sessions

Branch fix/desktop-settings-selection addresses disabled settings during VPN work.
The command port now admits bounded settings/export maintenance without replacing
an active cancellation token. The worker services it while awaiting the owned
operation. Ready/saving/exporting state prevents premature draft initialization,
repeat submissions and closing before accepted maintenance finishes.

- Settings now separates General, Presets and Connection. New preset is explicit;
  an editable frontend draft preserves changes across preset selection and allows
  immediate assignments. Save indicates dirty/invalid/saving/saved state; Discard
  restores saved preferences. Appearance previews without persisting until Save.
- Long press (550ms), right-click Select, Select all and Clear use visible relay
  models keyed by path. Watcher refreshes preserve marked paths. Filtered-out
  selections are dropped rather than silently included in an export.
- The export overlay disables underlying focus/input, captures selected rows,
  provides a folder picker and editable path, previews filenames, and offers
  name, JP, name_JP; full country names changes these to japan/name_japan.
  Name-only exports ignore the country toggle. Unknown countries are explicit.
- Batch size is bounded to 500, with up to 10,000 create-new attempts per filename.
  Collisions receive suffixes; no file is overwritten. Failed copies remove their
  incomplete output and report partial progress without deleting completed files.
- Saved tray mode keeps the process and VPN alive when closing. Missing hosts
  keep the window visible instead of quitting; host loss reopens it. Click only
  reopens; menu Connect, Disconnect/Stop, Settings and Quit use existing intents.
  A bounded worker updates menu state on connection/busy changes and menu opening.
- Headless real-component tests exercise preset creation/rename/assign/save,
  invalid edits/discard, long press/release, actual right-click menu selection,
  Select all, and export preview styles. Input contrast remains tested at 4.5:1.
- Native X11 smoke saves through the real appearance/name controls. Disposable
  D-Bus tests cover tray opt-in, disabled mode, host loss and second launch.
  A separate real-input test uses an isolated nmcli fixture: close/reopen,
  saving while connected, selected export to Saved_config_japan.ovpn while
  connected, tray Disconnect/Connect and Quit all pass. Export bytes match source.
- Initial native export harness clicks raced a layout transition; waiting for
  the view transition fixed the harness. Two consecutive session runs passed.
  A native startup timeout without a session bus prompted wrapping CI smoke in
  dbus-run-session and retaining exited-app stderr for useful failure reports.
- Actual renderer coverage is now 216 captures, adding Presets, Connection and
  Export at four sizes in light/dark. Reviewed narrow General/Presets and dark
  Export: pinned Save, visible labels and unclipped preview/actions. Long forms
  scroll at minimum size; no claim that every control fits without scrolling.
- No live NM/polkit/egress, GNOME/KDE, Wayland or macOS interaction proof is claimed.
- Final gates: 181 workspace Rust tests, 12 packaging tests, strict workspace
  all-targets/all-features clippy, fmt, shape/dups, source comments/budget and diff
  guards pass. Final native session smoke also verifies export while connected.
  Changes remain local; no push, release or main merge is authorized/performed.

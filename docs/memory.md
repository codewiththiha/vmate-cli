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

# Linux public relay evidence

## 2026-11-21: initial runner investigation

Commit f574028 ran on a real GitHub Ubuntu runner in a privileged Ubuntu 24.04
container with its own network. The container used Linux 7.0.0-1017-azure and
OpenVPN 2.6.19, OpenSSL 3.0.13. No private test server replaced built-in relays.

- Ordinary Linux and macOS CI passed:
  https://github.com/codewiththiha/vmate-cli/actions/runs/44894276675
- Live run failed:
  https://github.com/codewiththiha/vmate-cli/actions/runs/44894276641
- Evidence artifact: linux-public-vpngate-evidence (seven-day retention).
- Scanning 277 built-in UDP configs found two matches (JP and HK), exported
  them and saved them to SQLite.
- The JP terminal transcript displayed a connected status. The harness searched
  raw ANSI output for a contiguous success string and missed incremental TUI
  updates. It therefore reached its watchdog instead of sending interactive q.
- After watchdog shutdown the snapshot contained OpenVPN PID 2108 in D state,
  reparented to PID 1, and a host route to 60.73.48.68. No tun interface remained
  in the final link snapshot. D means uninterruptible sleep, not a zombie.
- This run does not prove q or restart cleanup. The later pyte-based harness
  renders terminal updates before interpreting status. A rerun is required.

A successful scan plus a connected TUI is not evidence of working Internet
traffic. No egress or DNS assertion was performed. Container destruction removes
the isolated test network but must not be counted as application cleanup.

## Remaining investigation

The signal path has two listeners: ConnectService cancellation and CLI
SignalGuard emergency exit. The latter force-kills groups and exits without
awaiting child reaping or graceful route removal. That competing owner needs
consolidation before claiming graceful SIGTERM behavior. A D-state process
cannot be guaranteed to exit by SIGKILL; record kernel and wait-channel details
when reproducible. No release or all-edge-case guarantee is justified by this run.

## Corrected terminal detection

Run https://github.com/codewiththiha/vmate-cli/actions/runs/44894691962
at commit 49ca11d used a rendered terminal screen rather than raw ANSI matching.
Linux and macOS CI passed in run 44894691984.

- Scan found the same JP/HK relay matches.
- Interactive q passed: connected status observed, exit 0, no OpenVPN processes,
  and route/link snapshots equal to the baseline.
- Interactive r then q passed: reconnect and subsequent connected status
  observed, exit 0, no OpenVPN processes, baseline routes/links restored.
- SIGTERM failed: OpenVPN PID 2159 remained in D state and the relay host route
  remained. This reproduced the competing emergency shutdown path.

The follow-up removes CLI SignalGuard and leaves ConnectService as the signal
owner. Cancellation awaits handle teardown and reaping; TuiGuard still restores
the terminal on normal return. Another live rerun is required before declaring
SIGTERM fixed. This does not claim kernel D-state immunity.

## Shutdown-owner fix: passed live rerun

Commit 9d970a5:

- Live Linux: https://github.com/codewiththiha/vmate-cli/actions/runs/44894924350
- Linux/macOS CI: https://github.com/codewiththiha/vmate-cli/actions/runs/44894924339
- Both workflows completed successfully.
- Linux 7.0.0-1017-azure; Ubuntu 24.04 container; OpenVPN 2.6.19.
- Built-in UDP scan found two passing relays.
- q, r followed by q, and SIGTERM each observed a connected session, exited 0,
  and passed the no-surviving-OpenVPN check.
- Each session's final route and interface snapshots exactly matched baseline
  when reviewed after artifact download.
- Persistent machine-readable evidence is in linux-public-relays-results.json;
  the linked workflow artifact additionally contains raw terminal transcripts.

The signal failure reproduced before the shutdown-owner fix and passed after it.
This is scoped evidence, not an all-edge-case guarantee. Internet egress, DNS,
network-loss recovery, SIGKILL, machine reboot and macOS live VPN teardown were
not validated. Bundled release variants are not implemented by this change.

## Rootless scans and independent input

At fe354bf, run
https://github.com/codewiththiha/vmate-cli/actions/runs/44898773192 passed.
The scan ran as UID/GID 65534, not root, and completed built-in public relay
handshakes while preserving routes, links, rules and resolver-file hash.
Interactive q, r reconnect followed by q, Ctrl+C, SIGTERM and SIGHUP all exited
0 with no OpenVPN processes and restored network state. Reconnect observed a
new process generation, not merely a transient UI notice. The terminal emulator
must retain its screen when observing incremental redraws; clearing only the
emulator is not a valid test of the real TUI.

Persistent evidence: rootless-public-relay-results.json. This does not measure
traffic throughput, DNS reachability or public-relay congestion. The 500-worker
resource stress uses a separate local sink rather than burdening public relays.
Subsequent local-error classification changes are checked by their own reruns.

## Final local-error/teardown invariant rerun

At 2a4c36e all three workflows passed:

- Public rootless scan and live lifecycle:
  https://github.com/codewiththiha/vmate-cli/actions/runs/44899043216
- 64/500-worker resource and shutdown cases:
  https://github.com/codewiththiha/vmate-cli/actions/runs/44899043086
- Linux/macOS lint, docs and 149 Rust tests:
  https://github.com/codewiththiha/vmate-cli/actions/runs/44899043097

Public q/r/Ctrl+C/SIGTERM/SIGHUP cases all exited 0, left no OpenVPN process and
restored network state. Reconnect replaced the original child process. Scan ran
unprivileged and preserved resolver hash and network state. At 500 local-sink
workers q/Ctrl+C/SIGTERM/SIGHUP cleanup took 175–193 ms, with no surviving children
or state changes. Machine-readable summaries identify their source commit/run.

The test suite also covers local TUN-permission errors preserving stored relay
history and a missing binary aborting scan. It does not claim complete log-error
classification, real managed/polkit verification, working Internet/DNS egress,
macOS live VPN teardown or immunity to SIGKILL/kernel/machine failures.

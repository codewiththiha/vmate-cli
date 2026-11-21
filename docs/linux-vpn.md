# Linux scan-to-connect investigation

## Confirmed code findings

1. Scan defaults to 15s, connection to 5s. A passing slow server can time out
   twice and be deleted from history. Compare with `--connect-timeout 20s`
   before blaming dead relays. Defaults are unchanged in this patch.
2. CleanupGuard incorrectly conditioned targeted cleanup on --killall.
   Fixed: targeted cleanup always runs; the global sweep remains opt-in.
3. PID registry only appended, retaining dead PIDs across the entire scan.
   Fixed: normal handle teardown forgets reaped children, avoiding stale sweeps.
4. OpenVPN children had no kill-on-drop fallback. Fixed for unexpected exits.
5. Terminal polling blocked async execution; noninteractive polling could spin
   without yielding. Fixed with an awaited timer and a zero-time terminal poll.
6. Completed scan tasks accumulated until scheduling ended. They are now drained.
7. Probe success deliberately skips route/interface configuration. It is not
   proof of working system routes, DNS, reachability, bandwidth or later uptime.

These are code findings, not a reproduction of the reported machine's outage.

## Why Linux and macOS may differ

Compare the same endpoint set, transport, OpenVPN version, workers, timeout,
privileges and network at the same time. Linux DCO/kernel/TUN support, file
limits, cipher/TLS policy, DNS integration and network filtering differ from
macOS. Public VPN Gate endpoints are a static list here; availability changes.
High --max settings can exhaust local descriptors or overwhelm remote relays.
Start with --max 8 or 16 and --timeout 20s, then measure, rather than increasing
concurrency blindly. Do not add retries without an explicit bound.

## Capture an actual failure

Record `uname -a`, `openvpn --version`, `vmate-cli doctor`, `ulimit -n`, exact
arguments and resolved settings. With no other VPN active, compare `ip addr`,
`ip route`, `ip rule`, and `resolvectl status` before/after. Inspect
`ps -eo pid,ppid,pgid,stat,args` to distinguish a live daemon from a zombie.
Use verbose connection output (v in the TUI); redact addresses and credentials.
Test one trusted config directly with OpenVPN and the same timeout. Check
whether failure is before handshake, during TUN setup, route installation,
DNS, or traffic. Never publish private keys or auth-user-pass contents.

## Important remaining limits

- Configs with daemon/log/syslog directives can detach or hide the output this
  program monitors. Reject or sanitize these explicitly in a separate change;
  command-line verbosity alone does not neutralize daemonization.
- Config scripts/plugins execute privileged code. Only use trusted configs.
- Hard kill cannot guarantee OS route/DNS rollback. Normal teardown remains
  graceful; abnormal-drop cleanup is only a process-liveness safety net.
- DNS integration is not provided merely by opening a TUN device.
- Local permission/route errors can still consume the per-config retry budget.
  Classifying systemic versus relay failures needs a dedicated outcome model.
- Dropping after persistent failures is existing behavior, not evidence that a
  relay permanently died. Preserve exports before diagnostic experiments.

## Bundling OpenVPN

A system package is not technically required: `--openvpn-bin /absolute/path`
already supports a private executable. A release could ship a platform-specific,
verified OpenVPN executable and its required libraries. It still needs host
TUN/utun support and appropriate privileges; bundling does not bypass them.

A bundled executable can create a system-level VPN just like a system-installed
one, provided routes and DNS are configured. A full tunnel requires suitable
redirect-gateway/routes; a split-tunnel config stays split. A userspace SOCKS
proxy is not an equivalent replacement for system-level routing.

Do not download or extract executables as root at runtime. Prefer signed release
artifacts, checksums, pinned upstream versions, an SBOM, security updates and a
license/source compliance review. OpenVPN 2.x is GPL software: distributing it
needs applicable notices and corresponding-source obligations. Embedding its
code into an MIT binary has different implications from shipping separate
executables. OpenVPN 3 Core integration would be an architectural/licensing
project, not a drop-in removal of the dependency. No binary is bundled here.

## References

- https://openvpn.net/community-docs/community-articles/openvpn-2-6-manual.html
  Documents route-noexec, ifconfig-noexec, daemon redirection and Linux DCO.
- https://docs.rs/tokio/latest/tokio/process/struct.Command.html
  Documents process groups and kill_on_drop ownership behavior.
- https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html
  Documents task result retention and cancellation on drop.
- https://www.gnu.org/licenses/gpl-faq.html
  Distribution/licensing guidance; not a substitute for legal review.

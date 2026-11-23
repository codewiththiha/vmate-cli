# Architecture and verification map

Application frontends live under apps/: cli and desktop (Rust + Slint).
crates/vmate-core owns VPN orchestration, storage, parsing and OS integration.
crates/vmate-engine composes application commands and bounded snapshots for the
GUI. Neither engine crate imports Slint, window handles, dialogs or clipboard.
The existing CLI calls core services directly; its terminal UI is not embedded
in the desktop frontend. NetworkManager session ownership is shared in core.

The engine Port is the replacement boundary for another GUI. Configuration
injects storage and executable paths. One owned Tokio worker serializes commands;
its 16-item channel rejects new work while busy. Cancellation bypasses that
channel and is installed before dispatch. Shutdown/SIGTERM cancels the active
operation, awaits core teardown and joins the worker thread. The GUI keeps the
window visible during teardown. Failed managed teardown shows unconfirmed state,
not a false disconnected guarantee. Snapshots cap relays at 500 and events at
128 entries of at most 2048 characters. Diagnostic display caps at 8192 characters.

Scan discovers paths, limits concurrent workers with a semaphore, launches
OpenVPN probes, resolves countries and persists successful handshakes. Probe
success is not an Internet connectivity guarantee. Connect loads candidates,
applies retry/cooldown policy, monitors handshake then connected output, and
owns the process group until teardown.

## Current changes

- One owned terminal thread feeds a bounded queue; quit has priority.
  Cancellation-safe input waits are independent of rate-limited log rendering.
- Targeted cleanup runs independently of the opt-in global killall sweep.
- Handles own a shared registry reference, unregister after reaping and kill
  their process group on unexpected drop. Child kill-on-drop is a second guard.
- Aborted probes do not persist connection failures.
- Scan drains completed JoinSet entries while scheduling and uses cancellation
  while waiting for a semaphore permit. Match-limit cancellation is a child
  token and cannot cancel its caller's token.
- Large builtin/connect inline test modules move to feature-local test files.
  Static endpoints move to builtin/remotes.rs; no exemption is needed.

## Verification boundaries

Unit/integration tests use shell processes or fakes, not external VPNs.
Linux sandbox tests cannot validate macOS utun, host DNS or route rollback.
Existing large functions are baselined, not represented as fully refactored.
The shape scanner is deliberately heuristic: braces in strings, macros and
complex signatures can mislead it. Review inline tests, routing matches and
comments manually. Existing duplicate bodies are recorded, not silently ignored.

## Follow-up isolation and privilege boundaries

Scan probes use dev null, route/ifconfig noexec, route-nopull and script-security
0. Policy preflight rejects includes/plugins/daemon/logging and unsupported
probe device/script overrides. Pure scan no longer elevates. Existing configs
are not rewritten unless the user requests --modify.

Signal listener ownership is shared through an abort-on-drop guard. q/Ctrl+C/SIGHUP
cancel scan workers; a canceled all-command does not hand matches to connect.
Geo waits have a five-second deadline and cancellation wake. Real connected
sessions receive the full graceful shutdown interval before force escalation.
Piped/noninteractive connect uses a console host, not a raw terminal.

Linux managed mode delegates an existing VPN profile to NetworkManager. It is
an explicit alternative, not an automatic backend switch or a newly installed
root daemon. See docs/privileges.md for authorization and crash limitations.

Local setup errors have a distinct monitor outcome. They abort raw connection
attempts without consuming relay retry budgets. Probe tester errors cancel the
scan, join workers and return an error rather than recording remote failures.
Teardown is checked before a successful probe or another tunnel is allowed.

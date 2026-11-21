# Architecture and verification map

The workspace has two crates: vmate-core owns VPN orchestration, persistence,
config parsing and OS integration; vmate-cli owns flags, wiring and terminal UI.
No UI, PDF renderer or parity-kit crate exists: those parts of the reference
policy are inapplicable and must not be scaffolded.

Scan discovers paths, limits concurrent workers with a semaphore, launches
OpenVPN probes, resolves countries and persists successful handshakes. Probe
success is not an Internet connectivity guarantee. Connect loads candidates,
applies retry/cooldown policy, monitors handshake then connected output, and
owns the process group until teardown.

## Current changes

- Terminal key polling waits on Tokio before a nonblocking terminal probe.
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

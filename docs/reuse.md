# Reuse index

Search with `git grep` before adding logic. Preserve engine/frontend boundaries.

| Responsibility | Existing owner |
|---|---|
| Spawn, merged logs, child teardown | vmate-core/src/ovpn/process.rs |
| Output classification | vmate-core/src/ovpn/monitor.rs, keywords.rs |
| Targeted process groups, registry, cleanup | vmate-core/src/system/killer.rs |
| Signal cancellation | vmate-core/src/system/signal.rs |
| Privileges and sudo storage ownership | vmate-core/src/system/root.rs |
| Bounded concurrent probes | vmate-core/src/scan/service.rs |
| Retry budget and candidate orchestration | vmate-core/src/connect/service.rs |
| Connected monitoring and persistence | vmate-core/src/connect/session.rs |
| Candidate cooldown and deferral | vmate-core/src/connect/queue.rs |
| Config identity and materialization | vmate-core/src/builtin/mod.rs |
| Persistent defaults | vmate-core/src/settings.rs |
| Country lookup and cache | vmate-core/src/geo/ |
| CLI scan/all pipeline | ../../apps/cli/src/commands/scan.rs |
| Terminal input and display | ../../apps/cli/src/ui/connect.rs |

Paths in the table are relative to crates/. Existing test fixtures stay in
feature test modules until there are two real cross-feature callers.

## Added shared owners

- `crates/vmate-core/src/system/signal.rs::cancel_on_shutdown`: abort-on-drop
  signal listener used by scan, raw connect and managed connect, including SIGHUP.
- `apps/cli/src/ui/input.rs::InputReader`: bounded dedicated terminal
  reader shared by scan and connect; quit bypasses the ordinary command queue.
- `apps/cli/src/ui/connect.rs::host`: TUI/console selection for raw,
  all and managed commands. Console mode never requests raw terminal state.
- `crates/vmate-core/src/ovpn/policy.rs::validate`: supervised-config policy
  shared by probe preflight, probe execution and the real connection runner.
- `tools/linux_state.py::network_state`: scan-isolation snapshots shared by
  public-relay and local high-worker test harnesses.
- `apps/cli/src/commands/managed.rs`: explicit existing-profile
  NetworkManager ownership; it does not introduce another privileged daemon.

- `crates/vmate-core/src/ovpn/monitor.rs::local_failure`: shared local-error
  recognition for probes, handshakes and established sessions.
- `crates/vmate-core/src/ovpn/process.rs::OpenVpnHandle::ensure_stopped`:
  post-teardown invariant checked before success or another connection attempt.

- `builtin/storage.rs::materialize`: reuse identical generated configs and
  atomically replace stale ones, repairing sudo ownership inside builtin storage.
- `tools/scan_upgrade.py`: Linux upgrade fixture for root-owned generated caches,
  failed stale-cache writes, sudo repair and subsequent normal-user scans.

## Desktop application owners

- `crates/vmate-core/src/managed.rs`: shared existing-profile activation,
  authorization errors and owned teardown for CLI and desktop.
- `crates/vmate-core/src/ovpn/binary.rs`: explicit/private/system executable
  resolution shared by CLI and the application engine; no runtime downloads.
- `crates/vmate-engine`: frontend-neutral commands, bounded state, cancellation
  and asynchronous service composition. Inject Configuration in engine tests.
- `apps/desktop/src/binding`: converts DTOs and callbacks only; no VPN processes.
- `apps/desktop/src/capture.rs`: headless real-component rasterization and
  pointer-navigation regression test. Fixtures never enter the engine database.
- `packaging/package.py`: system/bundled staging for both app frontends.
- `tools/review_screenshots.py`: verifies the capture matrix and embeds images
  in a self-contained gallery. Manual critique remains required.

- `crates/vmate-engine/src/automatic.rs`: recent-first retry coordinator; its
  backend contract is exercised by both the service adapter and scheduler tests.
- `platform.rs::recent`: bounded newest-first database query shared by refresh
  and automatic connection, rather than trusting a potentially stale UI model.
- `scan.rs::with_preset`: shared manual/automatic scan execution; action selection
  belongs to the engine, never the Slint callback layer.

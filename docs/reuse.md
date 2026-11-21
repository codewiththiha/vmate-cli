# Reuse index

Search with `git grep` before adding logic. No new crate is needed here.

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
| CLI scan/all pipeline | vmate-cli/src/commands/scan.rs |
| Terminal input and display | vmate-cli/src/ui/connect.rs |

Paths in the table are relative to crates/. Existing test fixtures stay in
feature test modules until there are two real cross-feature callers.

## Added shared owners

- `crates/vmate-core/src/system/signal.rs::cancel_on_shutdown`: abort-on-drop
  signal listener used by scan, raw connect and managed connect, including SIGHUP.
- `crates/vmate-cli/src/ui/input.rs::InputReader`: bounded dedicated terminal
  reader shared by scan and connect; quit bypasses the ordinary command queue.
- `crates/vmate-cli/src/ui/connect.rs::host`: TUI/console selection for raw,
  all and managed commands. Console mode never requests raw terminal state.
- `crates/vmate-core/src/ovpn/policy.rs::validate`: supervised-config policy
  shared by probe preflight, probe execution and the real connection runner.
- `tools/linux_state.py::network_state`: scan-isolation snapshots shared by
  public-relay and local high-worker test harnesses.
- `crates/vmate-cli/src/commands/managed.rs`: explicit existing-profile
  NetworkManager ownership; it does not introduce another privileged daemon.

- `crates/vmate-core/src/ovpn/monitor.rs::local_failure`: shared local-error
  recognition for probes, handshakes and established sessions.
- `crates/vmate-core/src/ovpn/process.rs::OpenVpnHandle::ensure_stopped`:
  post-teardown invariant checked before success or another connection attempt.

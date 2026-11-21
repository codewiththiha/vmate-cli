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

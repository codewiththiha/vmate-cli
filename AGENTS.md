# Agent working contract

Read README.md, docs/architecture.md and docs/reuse.md before editing.
Read docs/linux-vpn.md before modifying process or network ownership.

## Shape and reuse

- One responsibility per file. New files, including tests, stop at 500 lines.
- New functions stop at 80 lines. Existing ceilings in
  tools/shape_baseline.toml never grow; lower them when shrinking code.
- Exemptions require owner approval; no exemptions are currently declared.
- Keep facades small (target 200 lines); private children live under the facade.
  Use pub(super) at most for newly moved private internals. Keep public APIs stable.
- Move inline test modules over 150 lines to feature/tests/<topic>.rs.
- Command/option/property matches over 12 arms only route to child functions.
- Search first. Record searches in commit bodies and index shared items in
  docs/reuse.md. Extract on the second real caller, not speculatively.
- No utils/helpers/misc/common production modules, forwarding-only functions,
  speculative options, duplicate state, dead code, or TODO/FIXME scaffolding.
- Core logic belongs in vmate-core; terminal code belongs in vmate-cli.
  Integration fixtures belong under tests/common/mod.rs when shared.
- Separate move-only commits from behavior changes. No renaming or signature
  changes in move-only commits. Do not rewrite published history.

## Ownership

Every process, task, timer, registry and cache needs an owner and teardown.
Cancellation must wake blocked work. Reap children; forget PIDs after teardown.
Prefer graceful network teardown; SIGKILL is escalation, not route rollback.
Bound caches and queues. Drop stale results at the consumer boundary.
Never run a real VPN in CI or an agent sandbox without explicit approval.
Never use global killall by default. Treat configs as privileged input.

## Style and documentation

Use default rustfmt (100 columns) and clippy with -D warnings.
Docs state contracts, invariants, units, ordering and failure cases, not history.
First doc sentences fit 80 columns; item docs have at most 12 following lines,
module docs 20. Docs must not exceed the documented item.
Ordinary comments are single lines, <=80 columns and <=15 words; at most three
per function. Allow two per diff or one per 25 added code lines, whichever is
larger. Review manually: heuristic shape checks do not enforce these rules.
Do not add per-poll or per-frame logs. Record measured memory work in
 docs/memory.md; never claim optimization without workload and measurements.

## Verification and commits

```sh
git config core.hooksPath .githooks
python3 tools/shape_check.py --dups
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

Subjects stop at 50 characters. Changes over 50 added/deleted lines require
What:, Why:, Verification:, Search: sections. Performance commits also require
Command:, Machine:, Before:, After:. tools/commit_check.py enforces these gates.
Use the owner's requested email kt0071@gmail.com for newly created commits;
do not rewrite original authors, copyright, or invent the owner's name.
Do not store tokens, private configs or runtime logs in version control.

# Scan upgrade regression investigation

The 1.2.3 test coverage missed an existing root-owned built-in cache.
`materialize` overwrote every config on every scan, failing for readable but
root-owned files. It now reuses matching contents without writing, replaces
stale contents atomically and repairs generated file/directory ownership when
running under sudo. A stale non-writable directory still requires a one-time
ownership repair; the error includes a command. Scanning does not auto-elevate.
Generated-file symlinks are rejected rather than overwritten or followed.

The local-error classifier discarded all useful OpenVPN diagnostic text and
made any `Options error:` abort the whole scan. Server-pushed unknown options
are not necessarily fatal. `[PUSH-OPTIONS]` diagnostics now remain subject to
normal success/exit/timeout monitoring rather than aborting the scan globally.
Actual startup setup errors still abort, retaining their full diagnostic text.

## Local verification

- 154 Rust tests, including read-only reuse, atomic stale-file replacement,
  symlink rejection, pushed-option warnings and full startup error details.
- Linux OpenVPN 2.6.14, OpenSSL 3.5.9, unprivileged UID 1000.
- Built-in public scan, 277 configs, 100 workers, 5-second timeout: exit 0,
  38 successful handshakes. This does not prove Internet egress.
- Root-owned 277-config migration fixture: matching readable cache reused,
  stale unwriteable cache reports repair command, sudo run returns ownership,
  subsequent unprivileged scan succeeds. Fake AUTH_FAILED OpenVPN only.

The exact options error on the reporting machine was erased by 1.2.3 and is
not yet known. Do not claim its specific cause is reproduced or fully fixed.
The same full scan with the old classifier did not reproduce it locally.

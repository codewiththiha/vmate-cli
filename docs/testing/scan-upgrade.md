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

## OpenVPN 2.7 reproduction

An additional unprivileged scan with source-built OpenVPN 2.7.8 reproduced the
abort with the old classifier. The preserved full diagnostic was:

```
Options error: option 'dhcp-option' cannot be used in this context ([PUSH-OPTIONS])
```

With the corrected classifier, the same 277-relay scan exited 0 and reported
36 successful handshakes. This source build disabled DCO, LZO, LZ4 and PAM;
it is not a claim of testing every distributor build or the reporting machine.
The regression test uses the exact diagnostic. The public-relay workflow also
builds pinned OpenVPN 2.7.8 and scans all 277 configs as nobody, requiring at
least one handshake, full scan completion and unchanged network snapshots.

## Final GitHub runner verification

Code 5c9ff32 passed Linux/macOS CI run 44915489969 with 154 tests and the Linux
sudo upgrade fixture. Public lifecycle and OpenVPN 2.7 scan run 44915489991
passed. The 2.7 scan ran as UID 65534, completed all 277 configs with 34 successful
handshakes and preserved links, routes, rules and resolver-file hash.
Permanent summary: scan-upgrade-results.json. These are handshake and lifecycle
checks, not Internet egress validation or testing on the reporting machine.

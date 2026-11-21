# vmate-cli 1.2.4

## Fix scans after upgrading from sudo-based versions

- Reuse unchanged generated configs instead of rewriting root-owned files.
- Replace stale generated configs atomically.
- Return generated config files and directories to the invoking user during
  sudo runs. Refuse generated-file symlinks.
- If stale storage is not writable, report a one-time ownership repair command.

## Fix OpenVPN 2.7 scans aborting on pushed options

OpenVPN 2.7 can report `Options error: option dhcp-option cannot be used in this
context ([PUSH-OPTIONS])`. Version 1.2.3 incorrectly treated server-pushed option
diagnostics as local setup failures and aborted the entire scan. Monitoring
now waits for the actual handshake outcome instead. Real local setup failures
still abort safely, but retain the complete diagnostic rather than only the
words `Options error:`.

## Verification

154 tests and Linux/macOS lint/docs passed. Linux CI exercised a root-owned
277-config cache, sudo repair and a subsequent normal-user scan. An unprivileged
OpenVPN 2.7.8 public scan completed all 277 configs with 34 successful handshakes
and unchanged network snapshots. Existing public q/reconnect/Ctrl+C/TERM/HUP
lifecycle tests also passed. The compatibility source build disabled DCO, LZO,
LZ4 and PAM; this does not validate every distributor build or Internet egress.
See docs/testing/scan-upgrade.md for provenance and limits.

## Upgrade

Extract your platform archive and run its install.sh. OpenVPN remains a separate
system dependency. Scan normally without sudo. If an old cache still needs
ownership repair, run once as your normal user:

```sh
sudo chown -R "$(id -u):$(id -g)" "$HOME/.config/vmate-cli"
vmate-cli scan --max 100 -t 5 --verbose
```

Use your custom XDG_CONFIG_HOME path instead if configured. The 500-worker
request can still be reduced to fit the process file-descriptor limit; this is
intentional, not a scan failure. The default remains 64 workers.

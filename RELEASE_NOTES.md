# vmate-cli 1.2.3

## Shutdown and input responsiveness

- Prioritize q and Ctrl+C during concurrent scans and heavy OpenVPN output.
- Handle SIGTERM and terminal hangup through bounded child-process teardown.
- Validate teardown before retrying a connection or reporting probe success.
- Support non-interactive connection input without requiring raw terminal mode.

## Rootless scanning and managed connections

- Probe with `dev null` without configuring host tunnel interfaces or routes.
- Reject unsupported probe configuration directives and disable scripts.
- Preserve relay history on local setup errors; abort scans on tester errors.
- Add `vmate-cli managed UUID` for existing Linux NetworkManager VPN profiles.
  Authorization follows system NetworkManager/polkit policy.

## Verified scope

149 Rust tests passed with Linux/macOS lint and documentation checks. Linux
public-relay tests passed q, reconnect, Ctrl+C, SIGTERM and SIGHUP teardown.
Unprivileged scans preserved observed interfaces, routes, rules and resolver
hash. All eight 64/500-worker local-sink stress cases passed without survivors.
At 500 workers cleanup took 175–193 ms; child PSS was about 683–685 MiB.
The default remains 64 workers; 500 is an explicit resource-heavy option.

## Installation and limitations

Choose the archive matching your OS and architecture, extract it and run its
`install.sh`. Verify the archive using the published SHA256 checksums.
These archives do not bundle OpenVPN; install OpenVPN separately. Raw VPN
connections still need system privileges. Managed mode requires an existing
NetworkManager VPN profile, plugin, credentials and suitable authorization.

Live NetworkManager/polkit, macOS VPN teardown and Internet/DNS egress were not
validated by the Linux lifecycle tests. SIGKILL and kernel failure cannot
promise graceful cleanup. See docs/testing/linux-public-relays.md for evidence.

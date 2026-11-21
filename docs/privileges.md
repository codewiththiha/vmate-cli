# Privileges and system-managed VPN sessions

## Implemented modes

| Mode | Privilege model |
|---|---|
| `scan` | Unprivileged OpenVPN handshake, null device, no host VPN interface |
| `connect` / connecting `all` | Existing raw OpenVPN path, elevated once per run |
| `managed UUID` | Linux NetworkManager owns the VPN; vmate is not elevated |

A bundled OpenVPN executable has the same privileges as the system executable.
No setuid bit, blanket capabilities, password storage, NOPASSWD rule or custom
root daemon is installed by vmate.

## Linux desktop: use the established network service

Install your distribution's NetworkManager OpenVPN plugin, and import a trusted
config using your desktop network settings. Configuration creation may need
administrator authorization. Configure required credentials in the desktop
secret agent/keyring before using this command.

```sh
nmcli -f NAME,UUID,TYPE connection show
nmcli general permissions
vmate-cli managed YOUR_EXISTING_VPN_UUID
```

Replace the last argument with an actual UUID from the first command.
`managed` refuses non-VPN profiles and profiles already active before launch.
It activates the existing profile, supports q/Ctrl+C/SIGTERM/SIGHUP and r reconnect,
and explicitly deactivates it on normal return or error. It neither imports
arbitrary configs as root nor changes polkit rules. Its external commands have
bounded deadlines, with a best-effort emergency deactivation on unwinding.

Whether activation prompts is determined by NetworkManager/polkit and the
session's permissions. An active desktop user may already be authorized; a
headless/SSH user may not be. Missing permissions or secrets produce errors,
not a sudo fallback. A desktop authorization agent may need to be running.
Only use one controller per profile: another program concurrently activating
the same UUID is outside this command's ownership assumptions.

A service-managed VPN does not inherently disappear when its client is killed
with SIGKILL. vmate cannot handle that signal. Inspect/deactivate the profile
with NetworkManager if vmate or the machine crashes. SIGTERM, terminal hangup and interactive
quit use the explicit deactivation path. This is not a kill switch.

This backend has mocked lifecycle tests, not live NetworkManager/polkit
validation on every supported distribution. The real public-relay tests use
raw OpenVPN, not this backend. Do not confuse the two coverage claims.

## macOS

Apple's Personal VPN API covers IPsec/IKEv2, not arbitrary OpenVPN configs.
Custom packet-oriented protocols use a packet tunnel provider and its Network
Extension entitlement/deployment model. A plain unsigned Rust CLI cannot silently
acquire those OS permissions. A signed app/extension or an explicitly installed,
restricted privileged helper is a separate distribution/security project.

No macOS authorization bypass or incomplete root service is shipped here. The
existing raw OpenVPN path still uses sudo. Initial OS/user consent cannot be
honestly eliminated merely by embedding the OpenVPN executable.

## References

- https://networkmanager.dev/docs/api/latest/nmcli.html
- https://networkmanager.dev/docs/api/latest/NetworkManager.html
- https://developer.apple.com/documentation/networkextension/personal-vpn
- https://developer.apple.com/documentation/networkextension/packet-tunnel-provider

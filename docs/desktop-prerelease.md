# vmate Desktop — experimental alpha

Native Rust + Slint desktop preview, with the CLI included. Not a stable VPN
release. Public VPN Gate relays are untrusted third parties; no privacy, DNS-leak,
kill-switch, or uninterrupted-connectivity guarantee is made.

## Changes in alpha.2

- Removed the extra in-window menu strip and custom window-drag handler.
- Restored one system-owned title bar; content clicks no longer start window drags.
- Added an opt-in Linux system tray icon in Settings, disabled by default.
- With a working tray host, closing hides the window without stopping owned work.
  Use the icon/menu to reopen, open Settings, or Quit. Without a host, closing
  exits normally; host loss reopens the window to keep the app reachable.
- Native X11 pointer navigation, text entry, Save and window-control tests pass.
  Tray lifecycle tests use a disposable D-Bus host, not a GNOME/KDE desktop.

The tray requires a StatusNotifier/AppIndicator host. GNOME may need an extension.
It does not replace the normal taskbar entry or add autostart. macOS tray support
is not included. Wayland and real GNOME/KDE tray rendering remain unvalidated.

## Choose one edition

- **system**: uses your installed OpenVPN executable.
- **bundled**: includes private OpenVPN 2.7.8 and matching runtime libraries for
  vmate scans and CLI connections. It does not replace system OpenVPN.

**Important:** Linux GUI activation uses NetworkManager. Its OpenVPN plugin still
requires system OpenVPN, even in the bundled edition. The private bundle alone
is sufficient for scans and CLI use, not for NetworkManager-managed connections.
No privileged helper, setuid binary, password store, or sudoers grant is installed.

## Downloads and installation

Both editions contain the GUI, CLI, launcher, icon, installation guide and notices.
System and bundled native packages conflict; install only one edition.

| Platform | Assets | Installation |
|---|---|---|
| Arch Linux x86_64 | `.pkg.tar.zst` | `sudo pacman -U ./vmate-…pkg.tar.zst` |
| Ubuntu 24.04+ / compatible Debian | `.deb`, amd64 or arm64 | `sudo apt install ./vmate-…deb` |
| Fedora 42+ | `.rpm`, x86_64 or aarch64 | `sudo dnf install ./vmate-…rpm` |
| Other compatible Linux | `.tar.gz` | Extract and run `install.sh` as your user |
| macOS Intel / Apple Silicon | `.dmg` | Open the matching image and drag vmate to Applications |

The filenames use `amd64` for Linux x86_64 and `arm64` for Linux aarch64. Linux
binaries require glibc 2.39+ and the listed GUI runtime libraries; these are not
universal Linux or AppImage downloads. Arch Linux ARM is not an advertised target.
The Mac `-cli.tar.gz` asset contains the same application bundle, not just the CLI.

For Linux GUI connections, install the appropriate NetworkManager OpenVPN plugin
and run a desktop polkit agent. Home tries recent relays, scans if exhausted, then
activates through the system service. Live NetworkManager authorization, automatic
failover, managed-tunnel egress and DNS behavior still need independent validation.

## macOS limitations

The app is ad-hoc signed, **not Developer-ID signed or notarized**. Gatekeeper may
block it; follow your organization's approval policy. Do not disable Gatekeeper.
Automatic Home activation is not implemented on macOS. Scan/export work; import
into an existing Tunnelblick installation and connect there. The app does not
claim to own or report that externally managed tunnel.

## Verification and provenance

Check every download against `SHA256SUMS`. CI builds both editions on Linux
amd64/arm64 and macOS Intel/Apple Silicon, verifies payloads/private OpenVPN,
exercises Linux native window controls, shared CLI/GUI history and Slint bindings.
Native Linux installer smoke tests run in disposable distribution containers.
These checks are not evidence that every host's VPN authorization or routing works.

Rust dependency notices are in each package. Matching Rust crate source archives
and inventories are separate platform-specific release assets. Bundled OpenVPN
includes upstream source and build metadata. Bundled non-system libraries retain
license notices, matching distributor/Homebrew source archives and build recipes.
The source tag includes the packaging/build scripts. No independent legal or
supply-chain audit is claimed. Slint's own attribution is in Help → About vmate.

Please include architecture, edition, distribution, app diagnostics and precise
reproduction steps when reporting a problem. Redact credentials and private data.

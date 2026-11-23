# Desktop installation and authorization

This branch adds an experimental Slint desktop, not a stable signed release.
The GUI is Rust + Slint; there is no Swift application or embedded webview.

## Editions

Both editions contain vmate-desktop and vmate-cli. System uses installed OpenVPN.
Bundled includes private OpenVPN 2.7.8, its non-system shared libraries, upstream
source archive and license/provenance files. Resolver order: explicit CLI/env
path, private executable next to the app, then PATH. No runtime binary download.
NetworkManager uses its own OpenVPN plugin/executable, not the private bundle.
Bundling the executable does not bundle a permission grant or a VPN service.

## Linux

Packages: amd64/arm64 .deb and .rpm, x86_64 Arch .pkg.tar.zst, and .tar.gz. Install a .deb with
`sudo apt install ./vmate-*.deb`, or unpack the matching tar and run install.sh
as your user. The tar installer defaults to ~/.local and registers the launcher,
icon and CLI. `install.sh --uninstall` removes app files, not user settings or
system VPN profiles. Do not install system and bundled editions simultaneously.
Launch vmate from the desktop application menu; pin it to the taskbar if wanted.
The system owns the title bar, resize borders and window controls. There is no
extra in-window menu strip or client-area window-drag handler.

Settings → Show system tray icon is off by default. Save to enable it. With an
available StatusNotifier/AppIndicator host, closing hides only the window; scans
or connections continue. Click the tray icon or choose Open vmate to return.
Its menu also offers Settings and Quit vmate. Quit cancels owned work and exits.
Disabling the setting unregisters the icon. Without a working tray host, closing
exits normally; loss of the host reopens the window so vmate cannot become hidden
and unreachable. GNOME may require an AppIndicator extension; KDE usually has a
host. This does not control normal taskbar visibility or install an autostart job.

Setup detects Arch/Manjaro, Fedora and Debian-family package instructions.
Install NetworkManager's OpenVPN plugin and run your desktop polkit agent.
Connect from Home, or explicitly import a relay and choose its system profile. The
system may prompt for permission or missing secrets. No password is cached by
vmate and no sudoers rule is installed. Already-active profiles are refused.
Stop/disconnect/quit cancels the owned session and asks NetworkManager to stop it.
A hard kill can leave a service-owned connection running; use system controls.

Pure scans are unprivileged, use dev null and do not install system routes.
Raw CLI connect still needs explicit sudo and trusted configs. Avoid blanket
NOPASSWD grants, setuid executables or permanent capabilities on user-writeable
binaries. Old root-owned config caches may require the documented one-time chown.

## macOS

Packages: separate Apple Silicon and Intel .dmg images with vmate.app, plus a
CLI-accessible app archive. Drag vmate.app to Applications; launch through Finder
or the Dock. Native traffic lights are retained. Navigation and About live inside the app.
The optional system tray setting currently applies only to Linux.
CLI binary: /Applications/vmate.app/Contents/MacOS/vmate-cli.

The GUI delegates OpenVPN connection authorization to an existing Tunnelblick
installation. Install it from https://tunnelblick.net, open it once, then import
a relay from vmate. Finish authorization and connect in Tunnelblick. vmate does
not claim to own or report the state of that external tunnel. macOS first-party
managed connect is not implemented; scanning and export work independently.

Review artifacts are ad-hoc signed, NOT Developer-ID signed or notarized.
Gatekeeper may block downloaded builds. A public distribution needs a configured
Developer ID identity, hardened runtime, notarization and stapling on macOS CI.
Do not disable Gatekeeper globally. No credentials are collected in this branch.
Signing credentials and real Mac validation are release gates, not simulated.

## Review workflow

`cargo run -p vmate-desktop -- --capture-dir review/screenshots` runs the actual
Slint component with the software renderer, without DISPLAY or network access.
Every screenshot has a fixture banner. It covers every page, busy/empty/connected
states and missing-dependency/authorization failures at desktop and compact sizes.
Native system decorations are not captured by this headless content renderer.
Engine tests use isolated storage and shell fixtures, not public VPN traffic.

Slint 1.19 needs Rust 1.95; workspace development uses current stable. See
licensing-desktop.md before distributing GUI or bundled binaries publicly.

After installation, run `which -a vmate-cli` and `vmate-cli --version` to detect
an older /usr/local/bin installation shadowing the new binary. The installer
does not silently delete unrelated or administrator-owned installations.

Linux tar runtime requirements include fontconfig, libxkbcommon and its X11
integration, X11/XCB, Xcursor, Xrandr, Xi and Wayland client libraries. Prefer
the .deb on Ubuntu 24.04+ to resolve these automatically. On Arch/Manjaro,
install libxkbcommon-x11, libx11, libxcb, libxcursor, libxrandr, libxi and
fontconfig. The tar package does not claim to be a self-contained AppImage.

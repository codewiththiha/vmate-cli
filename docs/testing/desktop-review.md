# Desktop screenshot and behavior review

The GUI was rendered with Slint's real software renderer, not an HTML facsimile.
Every fixture capture is visibly labeled and never inserted into real history.
The external skill is /home/user/skills/desktop-craft/SKILL.md, outside this repo.
Its research sources and constraints were read before implementing the UI.

## Capture loop

Iteration 1 covered the six pages and empty, busy, connected, dependency and
permission-failure states at desktop and compact sizes. Review found oversized
secondary actions, cramped copy buttons, oversized numeric fields and a log
centered in a large empty viewport. Iteration 2 corrected those and added hostname
and country filtering. It exposed a details-panel overflow at the shorter height.

The final pass constrains button widths, pins details actions below an independently
scrollable body and adds explicit lower-content captures for setup and details.
Navigation and relay selection have keyboard focus; buttons have focus and disabled
states. The log starts at the top-left. Setup copy actions retain readable padding.
There are 28 PNGs: 14 scenarios at 1160×770 and 860×740. A self-contained HTML
gallery is generated for visual review. CI uploads captures, gallery and native
smoke output; a missing state or suspiciously small capture fails the matrix check.

## Behavior checks

162 Rust tests passed locally, including actual Slint pointer activation/navigation,
engine-only 277-config fake scans/export, immediate cancellation before dispatch,
running-scan shutdown/join and bounded logs/validation. Existing CLI regressions
remain in apps/cli/tests. No public VPN connection was requested for this GUI work.

A real winit X11 window was exercised under Xvfb + Openbox: WM_CLASS was vmate,
custom right-side minimize and maximize changed window-manager state, and the
close button exited 0. This uncovered a missing libxkbcommon-x11 runtime dependency,
which was added to Debian metadata and setup requirements. It does not validate
Wayland compositor behavior or prove how every desktop groups taskbar entries.

Linux amd64 system and bundled .deb/.tar.gz packages were built locally with debug
binaries for packaging validation. Their layouts, checksums, CLI startup, private
OpenVPN version, local-prefix install, edition switching and uninstall were checked.
They are not optimized release artifacts. Native CI builds release packages for
Linux amd64/arm64 and macOS Intel/Apple Silicon, both editions, as review artifacts.

## Remaining release gates

Real NetworkManager/polkit authorization and macOS Tunnelblick handoff need platform
validation. macOS first-party OpenVPN connect is deliberately not implemented; the
GUI never elevates itself. Native macOS traffic lights are outside headless content
captures and need a Mac review. Unsigned/ad-hoc-signed images are not notarized.
Slint attribution is present in Help → About. Bundled-library licensing/source
obligations still require redistribution review.
Do not call these review artifacts a finished public release or universal VPN proof.

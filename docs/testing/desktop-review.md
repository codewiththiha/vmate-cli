# Desktop screenshot and behavior review

The GUI was rendered with Slint's real software renderer, not an HTML facsimile.
Every fixture capture is visibly labeled and never inserted into real history.
The external skill is /home/user/skills/desktop-craft/SKILL.md, outside this repo.
Its research sources and constraints were read before implementing the UI.

## Capture loop

Earlier passes were insufficient: a narrow relay detail pane clipped long names,
copy-heavy headings competed with actions, and only two similar widths were used.
The Home revision replaces the initial dashboard with one primary action.

Home pass 1 exposed a blank power control caused by missing Unicode glyph coverage.
Pass 2 bundled an SVG, removed repeated status/footer prose, anchored settings Save,
and replaced the detail split with a single list. Pass 3 added 480×600 coverage;
an error banner pushed bottom navigation below the window, and setup commands
wrapped beside Copy into a too-narrow column. The final pass shrinks the narrow
Home control, reserves navigation, and stacks setup Copy below its command.

Final captures: review/home-final, 24 scenarios at 480×600, 860×740, 1160×770 and
1440×900 (96 images). Home covers idle, scanning, connecting, connected, retrying,
stopping, exhaustion and authorization error. Settings has top and bottom captures.
The self-contained review/gallery.html embeds all four sizes. These are fixture
captures, not evidence of a connected public VPN or native Mac chrome.

## Behavior checks

170 Rust tests passed locally, including actual Slint pointer activation/navigation,
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

A cross-context regression also checks the relay library while a VPN is connected:
its scan action is disabled, not mislabeled as Stop scan. Disconnect is available on Home and in Connections. A UI pointer test guards against starting a scan in that state.

## Automatic-flow verification boundary

Seven deterministic scheduler tests cover recent ordering, empty-history scanning,
bounded rescans, disconnect preservation, reconnect-to-next behavior, cancelled
scan suppression, and fatal service errors. Preset validation covers assignments,
duplicate names, resource bounds, loop bounds and serialization. Real-component
pointer tests exercise Home Connect/Cancel/Disconnect and disable restart during
teardown. These do not validate live NetworkManager authorization or tunnel egress.
The new automatic path has not been exercised against a real system VPN service.

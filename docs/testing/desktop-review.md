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

## CLI and desktop runner contract

The latest local suite has 171 tests. The new CLI integration keeps a desktop
engine open while the real CLI scans into its database. The engine observes the
external write via a two-second background poll, then scans itself; the CLI lists and
exports those results. The Slint test now uses the real binding and engine actor,
not only callback flags, to verify scan completion reaches its actual relay model.
External insertions/removals preserve selection by path or clear it. A pixel check
keeps the logo and wordmark visual centers within two pixels at the tested size.
The refreshed 96 captures are in review/runner-parity.

The history watcher reads successful relay history, not CLI-only presentation options.
CLI scan defaults and desktop named presets remain distinct. VMATE_DB is honored
by both frontends; explicit CLI --db must point at the GUI's chosen database to
share data. Raw CLI connect and desktop NetworkManager activation have different
privilege/service boundaries, so passing one cannot prove the other.

Prepared CI: workspace contracts on Linux/macOS, native Linux controls, screenshot
matrix, packaging, and built-in public VPN Gate CLI sessions plus desktop-engine
scan/history/export parity inside the disposable live-test container. The engine
probe explicitly reports tunnel_tested=false; it is not a NetworkManager tunnel
or egress test. No local fake is described as a live VPN test.

Execution blocker: pushing the new revision failed because the workspace lacks
GitHub authentication. No new hosted result is claimed. Public API inspection of
older run 45118973809 (commit 6a6d5c2) showed linux-review passed, Linux package
staging failed, and macOS artifact uploads failed. Failure logs require authorized
access; those old results cannot establish the current branch's packaging status.

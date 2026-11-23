# One-button connection revision

Home contains one primary Connect / Cancel / Disconnect control and short state.
Recent successful relays are attempted newest-first, without a preliminary scan.
An exhausted history triggers the assigned scan preset, then automatic activation.
A failed candidate is removed from successful history, not deleted from disk.
Auto-reconnect is optional; its configurable loop limit bounds rescan cycles.
Cancellation bypasses the command queue and includes owned session teardown.

Scan and Connect each select a saved named scan preset. Presets contain worker
count and handshake timeout; retry settings belong to the connection workflow.
All scan, service commands, timers and retries execute in the engine runtime.
Linux uses existing NetworkManager; unsupported macOS automatic activation must
fail explicitly rather than open Tunnelblick and pretend it connected.

Review Home at minimum, compact and wide sizes in idle, scanning, connecting,
retrying, connected, stopping, exhausted and dependency-error states. Secondary
pages must scroll without growing the window or horizontally clipping content.

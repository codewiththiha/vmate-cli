# vmate-cli v1.0.1

Prebuilt binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
arm64). Each zip contains the `vmate-cli` binary and `install.sh`, which copies
the binary onto your PATH.

## Connect

- Pressing `n` during the connection handshake now skips to the next config
  without removing it from history.
- A connection that stays up past a short stability window no longer counts
  its crash against the retry budget: a config that works and then hits a
  network blip is no longer deleted from the recent list after two crashes.
  Connect-then-crash configs are still retried once and dropped after two
  failed attempts.
- The connect UI shows the `.ovpn` file name and a "removed from recent list"
  notice instead of the full path.
- Fixed a stale "Connected successfully to X" message lingering after
  skipping to the next config or reconnecting.

## Completions

- `completions` now installs the script by default; pass `--print` to write
  the raw script to stdout.
- Fixed the bash activation hint and the unsupported-shell message to
  reference `--print`, matching the new default.

## Packaging

- Release binary is about 15% smaller (4.1 MB to 3.5 MB on Apple Silicon)
  via `panic = "abort"`, trimmed tokio features, and dropping the unused
  image crate.
- New release workflow builds macOS (`aarch64`, `x86_64`) and Linux
  (`x86_64`, `aarch64`) binaries automatically when a version tag is pushed,
  and attaches SHA256 checksums to the release.
- `install.sh` is bundled with each binary: extract the zip, run
  `sudo ./install.sh`, and `vmate-cli` is on your PATH. It can also replace
  an existing install, or remove it later with `sudo ./install.sh --uninstall`.

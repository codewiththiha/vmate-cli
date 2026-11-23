#!/usr/bin/env bash
set -euo pipefail
SOURCE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
if [ "${1:-}" = --uninstall ]; then
  rm -f "$PREFIX/bin/vmate-cli" "$PREFIX/bin/vmate-desktop"
  rm -f "$PREFIX/share/applications/vmate.desktop" "$PREFIX/share/icons/hicolor/scalable/apps/vmate.svg"
  rm -rf "$PREFIX/lib/vmate" "$PREFIX/share/doc/vmate"
  echo 'App removed. User settings and system VPN profiles were preserved.'
  exit 0
fi
mkdir -p "$PREFIX/bin" "$PREFIX/share/applications" "$PREFIX/share/icons/hicolor/scalable/apps" "$PREFIX/lib/vmate" "$PREFIX/share/doc"
cp "$SOURCE/usr/bin/vmate-cli" "$SOURCE/usr/bin/vmate-desktop" "$PREFIX/bin/"
cp -R "$SOURCE/usr/share/doc/vmate" "$PREFIX/share/doc/"
cp "$SOURCE/usr/share/icons/hicolor/scalable/apps/vmate.svg" "$PREFIX/share/icons/hicolor/scalable/apps/"
while IFS= read -r line; do
  case "$line" in
    Exec=*) printf 'Exec="%s/bin/vmate-desktop"\n' "$PREFIX" ;;
    *) printf '%s\n' "$line" ;;
  esac
done < "$SOURCE/usr/share/applications/vmate.desktop" > "$PREFIX/share/applications/vmate.desktop"
rm -rf "$PREFIX/lib/vmate/openvpn"
if [ -d "$SOURCE/usr/lib/vmate/openvpn" ]; then
  cp -R "$SOURCE/usr/lib/vmate/openvpn" "$PREFIX/lib/vmate/"
fi
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" || true
printf 'Installed to %s. Launch vmate from your application menu.\n' "$PREFIX"
echo 'For CLI use, ensure its bin directory is on PATH. No root grant was installed.'

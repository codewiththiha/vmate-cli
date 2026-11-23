#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="${1:?provide an absolute installation prefix}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
META="$(python3 -c 'import json,sys; x=json.load(open(sys.argv[1])); print(x["url"]); print(x["sha256"]); print(x["version"])' "$ROOT/packaging/openvpn.lock.json")"
URL="$(printf '%s\n' "$META" | sed -n '1p')"
SHA="$(printf '%s\n' "$META" | sed -n '2p')"
VERSION="$(printf '%s\n' "$META" | sed -n '3p')"
curl --fail --location --proto '=https' --tlsv1.2 "$URL" -o "$WORK/openvpn.tar.gz"
python3 -c 'import hashlib,sys; assert hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest()==sys.argv[2]' "$WORK/openvpn.tar.gz" "$SHA"
tar -xzf "$WORK/openvpn.tar.gz" -C "$WORK"
cd "$WORK/openvpn-$VERSION"
if [ "$(uname -s)" = Darwin ]; then
  export CPPFLAGS="-I$(brew --prefix lzo)/include -I$(brew --prefix lz4)/include ${CPPFLAGS:-}"
  export LDFLAGS="-L$(brew --prefix lzo)/lib -L$(brew --prefix lz4)/lib ${LDFLAGS:-}"
  export PKG_CONFIG_PATH="$(brew --prefix openssl@3)/lib/pkgconfig:$(brew --prefix lzo)/lib/pkgconfig:$(brew --prefix lz4)/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
fi
./configure --prefix="$PREFIX" --sbindir="$PREFIX/bin" \
  --disable-dco --disable-plugin-auth-pam --disable-systemd
make -j2
make install
mkdir -p "$PREFIX/share/licenses/openvpn"
cp COPYING COPYRIGHT.GPL AUTHORS "$PREFIX/share/licenses/openvpn/" 2>/dev/null || cp COPYING "$PREFIX/share/licenses/openvpn/"
cp "$WORK/openvpn.tar.gz" "$PREFIX/share/licenses/openvpn/openvpn-$VERSION.tar.gz"
cp "$ROOT/packaging/openvpn.lock.json" "$PREFIX/share/licenses/openvpn/source.json"
if [ "$(uname -s)" = Darwin ]; then
  dylibbundler -od -b -x "$PREFIX/bin/openvpn" -d "$PREFIX/lib" -p '@executable_path/../lib/'
  python3 "$ROOT/packaging/mac-sources.py" "$PREFIX"
else
  python3 "$ROOT/packaging/vendor-linux.py" "$PREFIX"
fi
"$PREFIX/bin/openvpn" --version > "$PREFIX/share/licenses/openvpn/build-version.txt"

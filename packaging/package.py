#!/usr/bin/env python3
"""Stage reviewable CLI+GUI system/bundled packages without publishing them."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import tarfile
import native

ROOT = Path(__file__).resolve().parents[1]


def copy(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def linux(args, stage):
    for binary in ('vmate-cli', 'vmate-desktop'):
        copy(args.binaries / binary, stage / 'usr/bin' / binary)
    copy(ROOT / 'packaging/linux/vmate.desktop', stage / 'usr/share/applications/vmate.desktop')
    copy(ROOT / 'apps/desktop/assets/vmate.svg', stage / 'usr/share/icons/hicolor/scalable/apps/vmate.svg')
    if args.variant == 'bundled':
        subprocess.run([str(ROOT / 'packaging/build-openvpn.sh'),
                        str(stage / 'usr/lib/vmate/openvpn')], check=True)
    shutil.copytree(ROOT / 'packaging/licenses', stage / 'usr/share/doc/vmate/licenses')
    for name in ('LICENSE', 'README.md'):
        copy(ROOT / name, stage / 'usr/share/doc/vmate' / name)
    if args.notices:
        copy(args.notices, stage / 'usr/share/doc/vmate/THIRD-PARTY-NOTICES.txt')
    copy(ROOT / 'docs/desktop-installation.md', stage / 'usr/share/doc/vmate/INSTALLATION.md')
    copy(ROOT / 'LICENSE', stage / 'usr/share/licenses/vmate/LICENSE')
    shutil.copytree(ROOT / 'packaging/licenses', stage / 'usr/share/licenses/vmate/dependencies')
    control = stage / 'DEBIAN/control'
    control.parent.mkdir()
    dependency = ', openvpn' if args.variant == 'system' else ''
    package = 'vmate-bundled' if args.variant == 'bundled' else 'vmate'
    conflict = 'vmate' if args.variant == 'bundled' else 'vmate-bundled'
    control.write_text(f'''Package: {package}
Version: {args.version}-{native.RELEASE}
Architecture: {args.arch}
Maintainer: codewiththiha <kt0071@gmail.com>
Conflicts: {conflict}
Provides: vmate (= {args.version})
Depends: libc6 (>= 2.39), libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, libx11-6, libx11-xcb1, libxcursor1, libxrandr2, libxi6, libxcb1, libwayland-client0, fonts-dejavu-core{dependency}
Recommends: network-manager-openvpn, xdg-desktop-portal
Description: OpenVPN relay library and system VPN frontend
''')
    subprocess.run(['dpkg-deb', '-Zgzip', '-z6', '--root-owner-group', '--build', str(stage),
                    str(args.output / f'{stage.name}.deb')], check=True)
    shutil.rmtree(control.parent)
    if args.native_formats:
        if args.arch == 'amd64':
            native.arch(args, stage)
        native.rpm(args, stage)
    copy(ROOT / 'packaging/linux/install.sh', stage / 'install.sh')
    with tarfile.open(args.output / f'{stage.name}.tar.gz', 'w:gz') as archive:
        archive.add(stage, arcname=stage.name)


def macos(args, stage):
    app = stage / 'vmate.app/Contents'
    for binary in ('vmate-cli', 'vmate-desktop'):
        copy(args.binaries / binary, app / 'MacOS' / binary)
    resources = app / 'Resources'
    resources.mkdir(parents=True)
    plist = {'CFBundleIdentifier': 'net.codewiththiha.vmate', 'CFBundleName': 'vmate',
             'CFBundleDisplayName': 'vmate', 'CFBundleExecutable': 'vmate-desktop',
             'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': args.version,
             'CFBundleVersion': args.version, 'NSHighResolutionCapable': True,
             'LSMinimumSystemVersion': '11.0', 'CFBundleIconFile': 'vmate.icns'}
    with (app / 'Info.plist').open('wb') as file:
        plistlib.dump(plist, file)
    subprocess.run(['python3', str(ROOT / 'packaging/macos/icon.py'), str(resources)], check=True)
    if args.variant == 'bundled':
        subprocess.run([str(ROOT / 'packaging/build-openvpn.sh'),
                        str(resources / 'openvpn')], check=True)
    shutil.copytree(ROOT / 'packaging/licenses', resources / 'licenses')
    if args.notices:
        copy(args.notices, resources / 'THIRD-PARTY-NOTICES.txt')
    copy(ROOT / 'LICENSE', resources / 'LICENSE')
    copy(ROOT / 'docs/desktop-installation.md', resources / 'INSTALLATION.md')
    subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(app.parent)], check=True)
    (stage / 'Applications').symlink_to('/Applications')
    subprocess.run(['hdiutil', 'create', '-volname', 'vmate', '-srcfolder', str(stage),
                    '-ov', '-format', 'UDZO', str(args.output / f'{stage.name}.dmg')], check=True)
    with tarfile.open(args.output / f'{stage.name}-cli.tar.gz', 'w:gz') as archive:
        archive.add(app.parent, arcname='vmate.app')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['linux', 'macos'], required=True)
    parser.add_argument('--arch', required=True)
    parser.add_argument('--variant', choices=['system', 'bundled'], required=True)
    parser.add_argument('--version', default='1.2.4')
    parser.add_argument('--native-formats', action='store_true')
    parser.add_argument('--notices', type=Path)
    parser.add_argument('--binaries', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output = args.output.resolve()
    args.binaries = args.binaries.resolve()
    stage = args.output / f'vmate-{args.version}-{args.platform}-{args.arch}-{args.variant}'
    if stage.exists():
        raise SystemExit(f'refusing to overwrite staging folder: {stage}')
    stage.mkdir(parents=True)
    {'linux': linux, 'macos': macos}[args.platform](args, stage)
    assets = [p for p in args.output.iterdir() if p.is_file() and p.name != "checksums.json"]
    sums = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in assets}
    (args.output / 'checksums.json').write_text(json.dumps(sums, indent=2) + '\n')


if __name__ == '__main__':
    main()

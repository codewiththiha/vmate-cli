"""Build Arch and RPM packages from the same verified Linux payload."""
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import time


def root_owner(entry):
    entry.uid = entry.gid = 0
    entry.uname = entry.gname = 'root'
    return entry


def arch(args, stage):
    architecture = {'amd64': 'x86_64', 'arm64': 'aarch64'}[args.arch]
    dependencies = ['glibc>=2.39', 'fontconfig', 'libxkbcommon', 'libxkbcommon-x11',
                    'libx11', 'libxcb', 'libxcursor', 'libxrandr', 'libxi', 'wayland', 'ttf-dejavu']
    if args.variant == 'system':
        dependencies.append('openvpn')
    name = 'vmate-bundled' if args.variant == 'bundled' else 'vmate'
    lines = [f'pkgname = {name}', 'pkgbase = vmate', f'pkgver = {args.version}-1',
             'pkgdesc = Experimental native OpenVPN relay desktop and CLI',
             'url = https://github.com/codewiththiha/vmate-cli',
             f'builddate = {int(os.environ.get("SOURCE_DATE_EPOCH", time.time()))}',
             'packager = codewiththiha <kt0071@gmail.com>',
             f'size = {sum(p.stat().st_size for p in stage.rglob("*") if p.is_file())}',
             f'arch = {architecture}', 'license = MIT', 'license = LicenseRef-Slint-Royalty-free-2.0']
    lines += ['conflict = vmate' if args.variant == 'bundled' else 'conflict = vmate-bundled']
    if args.variant == 'bundled':
        lines.append(f'provides = vmate={args.version}')
    lines.extend('depend = ' + item for item in dependencies)
    lines += ['optdepend = networkmanager-openvpn: GUI system VPN activation',
              'optdepend = xdg-desktop-portal: configuration export dialog']
    with tempfile.TemporaryDirectory() as temporary:
        metadata = Path(temporary) / '.PKGINFO'
        metadata.write_text('\n'.join(lines) + '\n')
        archive = Path(temporary) / 'package.tar'
        with tarfile.open(archive, 'w') as output:
            output.add(metadata, arcname='.PKGINFO', filter=root_owner)
            output.add(stage / 'usr', arcname='usr', filter=root_owner)
        subprocess.run(['zstd', '-T0', '-q', str(archive), '-o',
                        str(args.output / f'{stage.name}.pkg.tar.zst')], check=True)


def rpm(args, stage):
    architecture = {'amd64': 'x86_64', 'arm64': 'aarch64'}[args.arch]
    dependencies = ['glibc >= 2.39', 'fontconfig', 'libxkbcommon', 'libxkbcommon-x11',
                    'libX11', 'libxcb', 'libXcursor', 'libXrandr', 'libXi', 'libwayland-client', 'dejavu-sans-fonts']
    if args.variant == 'system':
        dependencies.append('openvpn')
    with tempfile.TemporaryDirectory() as temporary:
        top = Path(temporary)
        for name in ('BUILD', 'BUILDROOT', 'RPMS', 'SOURCES', 'SPECS', 'SRPMS'):
            (top / name).mkdir()
        specification = top / 'SPECS/vmate.spec'
        name = 'vmate-bundled' if args.variant == 'bundled' else 'vmate'
        conflict = 'vmate' if args.variant == 'bundled' else 'vmate-bundled'
        specification.write_text(f'''Name: {name}
Version: {args.version}
Release: 0.1.desktop
Summary: Experimental native OpenVPN relay desktop and CLI
License: MIT AND LicenseRef-Slint-Royalty-free-2.0
URL: https://github.com/codewiththiha/vmate-cli
BuildArch: {architecture}
AutoReqProv: no
Conflicts: {conflict}
Provides: vmate = {args.version}
Requires: {', '.join(dependencies)}
Recommends: NetworkManager-openvpn
Recommends: xdg-desktop-portal
%description
Native Slint GUI and CLI. Private bundles do not replace NetworkManager OpenVPN.
%install
mkdir -p %{{buildroot}}
cp -a "{stage}/usr" %{{buildroot}}/
%files
%defattr(-,root,root,-)
/usr/bin/*
/usr/share/applications/*
/usr/share/icons/hicolor/scalable/apps/*
/usr/share/doc/vmate
/usr/share/licenses/vmate
'''+('/usr/lib/vmate\n' if args.variant == 'bundled' else ''))
        subprocess.run(['rpmbuild', '-bb', '--target', architecture,
                        '--define', f'_topdir {top}', '--define', '_build_id_links none',
                        '--define', '__os_install_post %{nil}', str(specification)], check=True)
        packages = list((top / 'RPMS').rglob('*.rpm'))
        if len(packages) != 1:
            raise RuntimeError(f'Expected one RPM, got {packages}')
        shutil.copy2(packages[0], args.output / f'{stage.name}.rpm')

#!/usr/bin/env python3
"""Verify native payload parity and optionally install in disposable containers."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


def check(package, stage):
    with tempfile.TemporaryDirectory() as temporary:
        target = Path(temporary)
        command = (['dpkg-deb', '-x', str(package), str(target)] if package.suffix == '.deb'
                   else ['bsdtar', '-xf', str(package), '-C', str(target)])
        subprocess.run(command, check=True)
        for binary in ('vmate-cli', 'vmate-desktop'):
            expected = hashlib.sha256((stage / 'usr/bin' / binary).read_bytes()).hexdigest()
            actual = hashlib.sha256((target / 'usr/bin' / binary).read_bytes()).hexdigest()
            assert actual == expected, package
        bundled = stage.name.endswith('-bundled')
        assert (target / 'usr/lib/vmate/openvpn/bin/openvpn').exists() == bundled
        assert (target / 'usr/share/doc/vmate/THIRD-PARTY-NOTICES.txt').is_file()
    if package.suffix == '.rpm':
        dependencies = subprocess.check_output(['rpm', '-qp', '--requires', str(package)], text=True)
    elif package.suffix == '.deb':
        dependencies = subprocess.check_output(['dpkg-deb', '-f', str(package), 'Depends'], text=True)
    else:
        dependencies = subprocess.check_output(['bsdtar', '-xOf', str(package), '.PKGINFO'], text=True)
        dependencies = '\n'.join(line for line in dependencies.splitlines() if line.startswith('depend ='))
    assert ('openvpn' in dependencies.lower()) != stage.name.endswith('-bundled'), package


def install(package):
    file = '/packages/' + package.name
    if package.suffix == '.deb':
        image = 'ubuntu:24.04'
        command = f'apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends {file}'
    elif package.suffix == '.rpm':
        image = 'fedora:42'
        command = f'dnf install -y --setopt=install_weak_deps=False {file}'
    else:
        image = 'archlinux:base'
        command = f'pacman -Syu --noconfirm && pacman -U --noconfirm {file}'
    command += ' && vmate-cli --version && vmate-desktop --capture-dir /tmp/review'
    if '-bundled.' in package.name:
        command += ' && /usr/lib/vmate/openvpn/bin/openvpn --version'
    subprocess.run(['docker', 'run', '--rm', '-v', f'{package.parent}:/packages:ro',
                    image, 'bash', '-euc', command], check=True, timeout=900)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--install', action='store_true')
    args = parser.parse_args()
    for stage in sorted(args.directory.resolve().iterdir()):
        if not stage.is_dir() or not (stage / 'usr').exists():
            continue
        for suffix in ('.deb', '.rpm', '.pkg.tar.zst'):
            package = stage.parent / (stage.name + suffix)
            if not package.exists():
                if suffix == '.pkg.tar.zst' and '-arm64-' in stage.name:
                    continue
                raise RuntimeError(f'Missing native package: {package}')
            check(package, stage)
            if args.install:
                install(package)
    print('Native metadata and payload checks passed')


if __name__ == '__main__':
    main()

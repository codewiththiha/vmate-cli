#!/usr/bin/env python3
"""Copy non-glibc OpenVPN shared libraries into its private prefix."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


def package_owner(source):
    canonical = source.resolve()
    candidates = dict.fromkeys((source, canonical))
    if canonical.is_relative_to('/usr/lib'):
        candidates[Path('/') / canonical.relative_to('/usr')] = None
    owners = set()
    for candidate in candidates:
        result = subprocess.run(['dpkg-query', '-S', str(candidate)],
                                capture_output=True, text=True, check=False)
        if result.returncode:
            continue
        for line in result.stdout.splitlines():
            package, separator, filename = line.rpartition(': ')
            if separator and Path(filename).resolve() == canonical:
                owners.add(package)
    if len(owners) != 1:
        raise RuntimeError(f'expected one verified package owner for {source}: {owners}')
    return owners.pop()


def retain_source(prefix, package):
    identity = subprocess.check_output(['dpkg-query', '-W',
        '-f=${source:Package} ${source:Version}', package], text=True).split()
    if len(identity) != 2:
        raise RuntimeError(f'Missing source identity: {package}')
    destination = prefix / 'share/sources' / identity[0]
    if destination.exists():
        return
    destination.mkdir(parents=True)
    subprocess.run(['apt-get', 'source', '--download-only', '--only-source',
                    f'{identity[0]}={identity[1]}'], cwd=destination, check=True)
    (destination / 'BUILD.txt').write_text(
        f'Distributor source: {identity[0]} {identity[1]}\n'
        'Build with the matching Debian/Ubuntu source package and build dependencies.\n'
        'The .dsc and Debian patches are included alongside upstream archives.\n')


def main():
    prefix = Path(sys.argv[1]).resolve()
    executable = prefix / 'bin/openvpn'
    library = prefix / 'lib'
    library.mkdir(exist_ok=True)
    text = subprocess.check_output(['ldd', str(executable)], text=True)
    entries = re.findall(r'=> (/\S+)', text)
    manifest = []
    for name in entries:
        source = Path(name)
        if source.name.startswith(('libc.so', 'libm.so', 'libpthread.', 'libdl.', 'librt.', 'ld-linux')):
            continue
        target = library / source.name
        shutil.copy2(source, target)
        subprocess.run(['patchelf', '--set-rpath', '$ORIGIN', str(target)], check=True)
        manifest.append({'source': name, 'name': target.name,
                         'sha256': hashlib.sha256(target.read_bytes()).hexdigest()})
        package = package_owner(source)
        retain_source(prefix, package)
        notice = Path('/usr/share/doc') / package.split(':')[0] / 'copyright'
        if not notice.exists():
            raise RuntimeError(f'missing redistribution notice: {package}')
        destination = prefix / 'share/licenses' / package.replace(':', '-')
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copy2(notice, destination / 'copyright')
    subprocess.run(['patchelf', '--set-rpath', '$ORIGIN/../lib', str(executable)], check=True)
    (prefix / 'share/licenses/library-manifest.json').write_text(json.dumps(manifest, indent=2))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Copy non-glibc OpenVPN shared libraries into its private prefix."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


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
        package = subprocess.check_output(['dpkg-query', '-S', str(source.resolve())], text=True)
        package = package.split(': /')[0].splitlines()[0]
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

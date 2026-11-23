#!/usr/bin/env python3
"""Check staged review packages and execute private OpenVPN without networking."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    directory = Path(sys.argv[1])
    for name, digest in json.loads((directory / 'checksums.json').read_text()).items():
        assert hashlib.sha256((directory / name).read_bytes()).hexdigest() == digest, name
    for stage in directory.iterdir():
        if not stage.is_dir():
            continue
        mac = (stage / 'vmate.app').exists()
        binaries = stage / 'vmate.app/Contents/MacOS' if mac else stage / 'usr/bin'
        for binary in ('vmate-cli', 'vmate-desktop'):
            assert (binaries / binary).is_file(), str(binaries / binary)
        version = subprocess.check_output([str((binaries / 'vmate-cli').resolve()), '--version'], text=True)
        assert 'vmate-cli' in version
        if stage.name.endswith('-bundled'):
            prefix = stage / ('vmate.app/Contents/Resources/openvpn' if mac else 'usr/lib/vmate/openvpn')
            result = subprocess.check_output([str((prefix / 'bin/openvpn').resolve()), '--version'], text=True)
            assert 'OpenVPN 2.7.8' in result
            assert (prefix / 'share/licenses/openvpn/openvpn-2.7.8.tar.gz').is_file()
            if not mac:
                linked = subprocess.check_output(['ldd', str(prefix / 'bin/openvpn')], text=True)
                assert 'not found' not in linked, linked
        if mac:
            subprocess.run(['codesign', '--verify', '--deep', '--strict', str(stage / 'vmate.app')], check=True)
    print('Package structure, checksums and private executable checks passed')


if __name__ == '__main__':
    main()

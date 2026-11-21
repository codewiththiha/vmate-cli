#!/usr/bin/env python3
"""Exercise Linux sudo-cache migration with synthetic OpenVPN failures."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    binary = str(Path(sys.argv[1]).resolve())
    uid, gid = os.getuid(), os.getgid()
    if uid == 0:
        sys.exit('run as the normal runner user with passwordless sudo')
    with tempfile.TemporaryDirectory(prefix='vmate-upgrade-') as temporary:
        root = Path(temporary)
        xdg = root / 'config'
        fake = root / 'openvpn'
        fake.write_text('#!/bin/sh\necho AUTH_FAILED\n')
        fake.chmod(0o755)
        env = os.environ | {'XDG_CONFIG_HOME': str(xdg), 'VMATE_NO_ELEVATE': '1'}
        command = [binary, '--db', str(root / 'db'), '--openvpn-bin', str(fake),
                   'scan', '--max', '4', '--no-save']

        def scan(elevated=False):
            args = command
            if elevated:
                args = ['sudo', '-n', 'env', f'XDG_CONFIG_HOME={xdg}',
                        f'SUDO_UID={uid}', f'SUDO_GID={gid}', *command]
            return subprocess.run(args, env=env, capture_output=True, text=True)

        first = scan()
        assert first.returncode == 0, first.stderr
        builtin = xdg / 'vmate-cli' / 'builtin'
        files = sorted(builtin.rglob('*.ovpn'))
        assert len(files) == 277, len(files)
        for path in files:
            path.chmod(0o644)
        subprocess.run(['sudo', '-n', 'chown', '-R', '0:0', str(builtin)], check=True)
        try:
            reused = scan()
            assert reused.returncode == 0, reused.stderr
            subprocess.run(['sudo', '-n', 'sh', '-c', 'printf stale > "$1"',
                            'fixture', str(files[0])], check=True)
            denied = scan()
            assert denied.returncode != 0
            assert 'sudo chown' in denied.stderr, denied.stderr
            repaired = scan(elevated=True)
            assert repaired.returncode == 0, repaired.stderr
            for path in [builtin, *builtin.rglob('*')]:
                assert path.stat().st_uid == uid, str(path)
            final = scan()
            assert final.returncode == 0, final.stderr
            print(json.dumps({'matching_root_owned_cache': 'passed',
                              'stale_cache_repair_hint': 'passed',
                              'sudo_returns_ownership': 'passed',
                              'normal_scan_after_sudo': 'passed',
                              'configs': len(files), 'uid': uid}))
        finally:
            subprocess.run(['sudo', '-n', 'chown', '-R', f'{uid}:{gid}',
                            str(builtin)], check=True)



if __name__ == '__main__':
    main()

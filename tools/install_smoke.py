#!/usr/bin/env python3
"""Test both Linux tar editions in a spaced local prefix, then uninstall."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    base = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix='vmate install ') as temporary:
        prefix = Path(temporary) / 'local'
        env = os.environ | {'PREFIX': str(prefix), 'XDG_CONFIG_HOME': str(Path(temporary) / 'config')}
        for variant in ('bundled', 'system'):
            stages = [p for p in base.iterdir() if p.is_dir() and p.name.endswith('-' + variant)]
            assert len(stages) == 1, stages
            stage = stages[0]
            subprocess.run(['bash', str(stage / 'install.sh')], env=env, check=True)
            assert (prefix / 'bin/vmate-desktop').is_file()
            assert (prefix / 'lib/vmate/openvpn').exists() == (variant == 'bundled')
            assert (prefix / 'share/doc/vmate/licenses/Slint-Royalty-free-2.0.md').is_file()
            desktop = (prefix / 'share/applications/vmate.desktop').read_text()
            assert f'Exec="{prefix}/bin/vmate-desktop"' in desktop
            command = [str(prefix / 'bin/vmate-cli'), '--db', str(Path(temporary) / 'history.db'), 'doctor']
            result = subprocess.check_output(command, env=env, text=True)
            if variant == 'bundled':
                assert '/lib/vmate/openvpn/bin/openvpn' in result
            subprocess.run([str(prefix / 'bin/vmate-cli'), '--version'], check=True)
        subprocess.run(['bash', str(stage / 'install.sh'), '--uninstall'], env=env, check=True)
        assert not (prefix / 'bin/vmate-desktop').exists()
        assert (Path(temporary) / 'history.db').exists()
    print('Installer switching, spaced launcher path, licensing, binary resolution and uninstall passed')


if __name__ == '__main__':
    main()

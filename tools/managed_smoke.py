#!/usr/bin/env python3
"""Check managed-session teardown against an isolated nmcli fixture."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

UUID = '01234567-89ab-cdef-0123-456789abcdef'


def wait_for(predicate):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError('Fixture state deadline exceeded')


def exercise(binary, external):
    with tempfile.TemporaryDirectory() as name:
        root = Path(name)
        active = root / 'active'
        nmcli = root / 'nmcli'
        nmcli.write_text('''#!/usr/bin/env python3
import os
from pathlib import Path
import sys
import time
root = Path(os.environ['NM_FIXTURE'])
args = sys.argv[1:]
if 'connection.type' in args:
    print('vpn')
elif '--active' in args:
    if (root / 'active').exists(): print(os.environ['NM_UUID'])
elif 'up' in args:
    (root / 'active').touch()
elif 'down' in args:
    (root / 'active').unlink(missing_ok=True)
    time.sleep(8)
''')
        nmcli.chmod(0o755)
        env = os.environ | {'PATH': f'{root}:{os.environ["PATH"]}',
                            'NM_FIXTURE': name, 'NM_UUID': UUID}
        process = subprocess.Popen([binary, 'managed', UUID, '--db', str(root / 'db')],
                                   env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            wait_for(active.exists)
            time.sleep(0.3)
            start = time.monotonic()
            if external:
                active.unlink()
            else:
                process.send_signal(signal.SIGTERM)
            stdout, stderr = process.communicate(timeout=5)
            elapsed = time.monotonic() - start
            assert process.returncode == 0, (stdout, stderr)
            assert not active.exists()
            assert elapsed < 4, elapsed
            return round(elapsed, 3)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


def main():
    binary = str(Path(sys.argv[1]).resolve())
    print(json.dumps({'fixture_cancel_seconds': exercise(binary, False),
                      'fixture_external_disconnect_seconds': exercise(binary, True),
                      'real_vpn_tested': False}))


if __name__ == '__main__':
    main()

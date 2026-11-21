#!/usr/bin/env python3
"""Check quit delivery while a fake VPN continuously floods output."""
import argparse
import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path


def exercise(binary, action, directory):
    fake = directory / 'openvpn'
    fake.write_text("#!/bin/sh\nprintf 'Initialization Sequence Completed\\n'\nwhile :; do printf 'busy handshake output stream\\n'; done\n")
    fake.chmod(0o700)
    config = directory / 'jp.ovpn'
    config.write_text('client\ndev tun\nremote 127.0.0.1 11999\n')
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
    process = subprocess.Popen([str(binary.resolve()), '--no-elevate', '--db', str(directory / 'db'),
                                '--openvpn-bin', str(fake), 'connect', str(config)],
                               stdin=slave, stdout=slave, stderr=slave, env=dict(os.environ, TERM='xterm'))
    os.close(slave)
    try:
        started = time.monotonic()
        while time.monotonic() - started < 0.75 and process.poll() is None:
            if select.select([master], [], [], 0.01)[0]:
                os.read(master, 65536)
        if process.poll() is not None:
            raise RuntimeError('CLI exited before input test')
        sent = time.monotonic()
        if action == 'TERM':
            process.send_signal(signal.SIGTERM)
        else:
            os.write(master, b'q' if action == 'q' else b'\x03')
        process.wait(timeout=5)
        elapsed = time.monotonic() - sent
        if process.returncode != 0 or elapsed > 1:
            raise RuntimeError(f'{action}: exit={process.returncode}, latency={elapsed:.3f}s')
        return {'action': action, 'signal_to_exit_seconds': elapsed, 'exit_code': process.returncode}
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    args = parser.parse_args()
    results = []
    with tempfile.TemporaryDirectory() as temp:
        for action in ('q', 'CTRL_C', 'TERM'):
            directory = Path(temp) / action
            directory.mkdir()
            results.append(exercise(args.binary, action, directory))
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

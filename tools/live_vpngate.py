#!/usr/bin/env python3
"""Exercise built-in public relays inside a disposable privileged container."""
import json
import fcntl
import struct
import termios
import os
import pty
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

REPORT = Path('/evidence')
BIN = '/workspace/target/debug/vmate-cli'
ENV = dict(os.environ, HOME='/tmp/vmate-live', TERM='xterm-256color')


def snapshot():
    result = {}
    for name, command in {
        'routes': ['ip', '-j', 'route'],
        'links': ['ip', '-j', 'link'],
        'processes': ['ps', '-eo', 'pid,ppid,pgid,stat,args'],
    }.items():
        result[name] = subprocess.check_output(command, text=True)
    return result


def no_children():
    result = subprocess.run(['pgrep', '-x', 'openvpn'], capture_output=True, text=True)
    if result.returncode != 1:
        raise RuntimeError(f'OpenVPN processes survived: {result.stdout}')


def session(config, action):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    command = [BIN, '--db', '/tmp/live.db', 'connect', str(config),
               '--connect-timeout', '25s', '--retry-count', '1']
    process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env=ENV)
    os.close(slave)
    output = bytearray()
    connected = False
    restarted = action != 'r'
    saw_reconnecting = False
    deadline = time.monotonic() + 100
    try:
        while time.monotonic() < deadline and process.poll() is None:
            if not select.select([master], [], [], 0.5)[0]:
                continue
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            output.extend(data)
            if connected and action == 'r' and b'Reconnecting to' in output:
                saw_reconnecting = True
                output.clear()
            if b'Connected successfully' not in output:
                continue
            if not connected:
                connected = True
                if action == 'r':
                    os.write(master, b'r')
                    output.clear()
                elif action == 'TERM':
                    process.send_signal(signal.SIGTERM)
                else:
                    os.write(master, b'q')
            elif not restarted and saw_reconnecting and b'Connected successfully' in output:
                restarted = True
                os.write(master, b'q')
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
        process.wait(timeout=10)
        if not connected or not restarted:
            raise RuntimeError(f'{action}: public relay did not complete required handshake(s)')
        no_children()
        return {'action': action, 'connected': connected, 'restarted': restarted,
                'exit_code': process.returncode, 'after': snapshot()}
    finally:
        REPORT.joinpath(f'session-{action}.log').write_bytes(output)
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


def main():
    REPORT.mkdir(parents=True, exist_ok=True)
    results = {'before': snapshot(), 'status': 'incomplete', 'sessions': []}
    try:
        no_children()
        command = [BIN, '--db', '/tmp/live.db', 'scan', '--max', '8',
                   '--limit', '2', '--timeout', '15s', '--export', '/tmp/matches']
        with REPORT.joinpath('scan.log').open('wb') as log:
            process = subprocess.Popen(command, env=ENV, stdout=log, stderr=log)
            try:
                process.wait(timeout=240)
            except subprocess.TimeoutExpired:
                process.send_signal(signal.SIGTERM)
                process.wait(timeout=15)
        no_children()
        configs = list(Path('/tmp/matches').glob('*.ovpn'))
        if not configs:
            raise RuntimeError('INCONCLUSIVE: no built-in public relay passed scanning')
        for action in ('q', 'r', 'TERM'):
            results['sessions'].append(session(configs[0], action))
        results['status'] = 'passed'
    except Exception as error:
        results['status'] = 'failed_or_inconclusive'
        results['error'] = str(error)
    finally:
        results['after'] = snapshot()
        REPORT.joinpath('results.json').write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))
    return results['status'] != 'passed'


if __name__ == '__main__':
    sys.exit(main())

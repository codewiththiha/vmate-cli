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
import pyte
from pathlib import Path
from linux_state import network_state

REPORT = Path('/evidence')
BIN = '/workspace/target/debug/vmate-cli'
ENV = dict(os.environ, HOME='/tmp/vmate-live', TERM='xterm-256color')


class RelayUnavailable(RuntimeError):
    pass


def snapshot():
    result = network_state()
    result['processes'] = subprocess.check_output(['ps', '-eo', 'pid,ppid,pgid,stat,args'], text=True)
    return result


def no_children():
    result = subprocess.run(['pgrep', '-x', 'openvpn'], capture_output=True, text=True)
    if result.returncode != 1:
        raise RuntimeError(f'OpenVPN processes survived: {result.stdout}')


def vpn_pids():
    result = subprocess.run(['pgrep', '-x', 'openvpn'], capture_output=True, text=True)
    return set(result.stdout.split())


def session(config, action, expires):
    before = network_state()
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    database = f'/tmp/session-{action}-{config.stem}.db'
    command = [BIN, '--db', database, 'connect', str(config),
               '--connect-timeout', '25s', '--retry-count', '1']
    process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env=ENV)
    os.close(slave)
    output = bytearray()
    screen = pyte.Screen(120, 40)
    stream = pyte.Stream(screen)
    connected, restarted = False, action != 'r'
    original_pids, new_pids = set(), set()
    generation_seen_at = None
    deadline = min(time.monotonic() + 70, expires)
    try:
        while time.monotonic() < deadline and process.poll() is None:
            if not select.select([master], [], [], 0.5)[0]:
                continue
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            output.extend(data)
            stream.feed(data.decode("utf-8", errors="replace"))
            up = screen.display[0].strip().startswith('Connected:')
            if connected and action == 'r' and not restarted:
                current = vpn_pids()
                if current and current.isdisjoint(original_pids):
                    new_pids = current
                    if generation_seen_at is None:
                        generation_seen_at = time.monotonic()
                    elif up and time.monotonic() - generation_seen_at >= 0.2:
                        restarted = True
                        os.write(master, b'q')
            elif up and not connected:
                connected = True
                original_pids = vpn_pids()
                if action in ('TERM', 'HUP'):
                    process.send_signal(signal.SIGHUP if action == 'HUP' else signal.SIGTERM)
                else:
                    os.write(master, {'r': b'r', 'CTRL_C': b'\x03'}.get(action, b'q'))
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
        process.wait(timeout=10)
        no_children()
        if network_state() != before:
            raise RuntimeError(f'{action}: network state not restored')
        if not connected or not restarted:
            raise RelayUnavailable(f'{action}: public relay did not complete required handshake(s)')
        return {'action': action, 'connected': connected, 'restarted': restarted,
                'original_pids': sorted(original_pids), 'new_pids': sorted(new_pids),
                'exit_code': process.returncode, 'after': snapshot()}
    finally:
        REPORT.joinpath(f'session-{action}-{config.stem}.log').write_bytes(output)
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


def main():
    REPORT.mkdir(parents=True, exist_ok=True)
    results = {'before': snapshot(), 'status': 'incomplete', 'sessions': []}
    try:
        no_children()
        scan_before = network_state()
        command = [BIN, '--db', '/tmp/live.db', 'scan', '--max', '8',
                   '--limit', '4', '--timeout', '15s', '--export', '/tmp/matches']
        with REPORT.joinpath('scan.log').open('wb') as log:
            process = subprocess.Popen(command, env=ENV, stdout=log, stderr=log, user=65534, group=65534)
            try:
                process.wait(timeout=240)
            except subprocess.TimeoutExpired:
                process.send_signal(signal.SIGTERM)
                process.wait(timeout=15)
        no_children()
        results['scan_network_unchanged'] = network_state() == scan_before
        if not results['scan_network_unchanged']:
            raise RuntimeError('scan changed system network state')
        configs = sorted(Path('/tmp/matches').glob('*.ovpn'), key=lambda path: ('_JP' not in path.name, path.name))
        if not configs:
            raise RuntimeError('INCONCLUSIVE: no built-in public relay passed scanning')
        expires = time.monotonic() + 480
        for action in ('q', 'r', 'CTRL_C', 'TERM', 'HUP'):
            for config in configs[:3]:
                if time.monotonic() >= expires:
                    raise RelayUnavailable('live test budget expired')
                try:
                    results['sessions'].append(session(config, action, expires))
                    break
                except RelayUnavailable as error:
                    results.setdefault('unavailable_relays', []).append(str(error))
            else:
                raise RelayUnavailable(f'{action}: no available relay completed the test')
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

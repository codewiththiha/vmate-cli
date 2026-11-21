#!/usr/bin/env python3
"""Measure real OpenVPN probes against a local UDP sink, never public relays."""
import argparse
import json
import os
import platform
import pty
import re
import resource
import signal
import socket
import subprocess
import threading
import time
from pathlib import Path
from linux_state import network_state


def memory(pid):
    try:
        status = Path(f'/proc/{pid}/status').read_text()
        rss = int(re.search(r'^VmRSS:\s+(\d+)', status, re.M)[1])
        return rss
    except (OSError, TypeError):
        return 0


def pss(pid):
    try:
        value = re.search(r'^Pss:\s+(\d+)', Path(f'/proc/{pid}/smaps_rollup').read_text(), re.M)
        return int(value[1])
    except (OSError, TypeError):
        return 0


def children(pid):
    try:
        return sorted({int(value) for task in Path(f'/proc/{pid}/task').iterdir()
                       for value in (task / 'children').read_text().split()})
    except OSError:
        return []


def drain(sock, stop):
    sock.settimeout(0.1)
    while not stop.is_set():
        try:
            sock.recvfrom(65536)
        except TimeoutError:
            pass


def benchmark(args, workers, quit_key):
    folder = args.output / f'{workers}-{quit_key}'
    folder.mkdir(parents=True, exist_ok=True)
    template = re.sub(r'(?m)^remote\s+.*$', f'remote 127.0.0.1 {args.port}', args.template.read_text())
    configs = folder / 'configs'
    configs.mkdir(exist_ok=True)
    for index in range(workers):
        (configs / f'jp_{index:04}.ovpn').write_text(template)
    command = [str(args.binary.resolve()), '--no-elevate', '--openvpn-bin', args.openvpn,
               '--db', str(folder / 'scan.db'), 'scan', str(configs),
               '--max', str(workers), '--limit', str(workers + 1), '--timeout', '60s', '--no-save']
    master, slave = pty.openpty()
    network_before = network_state()
    network_unchanged = True
    before = time.monotonic()
    process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                               env=dict(os.environ, TERM='xterm-256color'))
    os.close(slave)
    peak_parent = peak_children = peak_count = peak_pss = 0
    last_children = []
    samples = []
    try:
        while process.poll() is None and time.monotonic() - before < 15:
            network_unchanged &= network_state() == network_before
            last_children = children(process.pid)
            parent_rss = memory(process.pid)
            child_rss = sum(memory(pid) for pid in last_children)
            peak_parent = max(peak_parent, parent_rss)
            peak_children = max(peak_children, child_rss)
            peak_count = max(peak_count, len(last_children))
            samples.append([round(time.monotonic() - before, 3), len(last_children), parent_rss, child_rss])
            if len(last_children) >= workers:
                time.sleep(1)
                peak_pss = sum(pss(pid) for pid in last_children)
                peak_children = max(peak_children, sum(memory(pid) for pid in last_children))
                break
            time.sleep(0.1)
        sent = time.monotonic()
        if quit_key in ('TERM', 'HUP'):
            process.send_signal(signal.SIGHUP if quit_key == 'HUP' else signal.SIGTERM)
        else:
            os.write(master, b'q' if quit_key == 'q' else b'\x03')
        process.wait(timeout=12)
        teardown = time.monotonic() - sent
        survivors = [pid for pid in last_children if Path(f'/proc/{pid}').exists()]
        return {'workers_requested': workers, 'peak_live_children': peak_count,
                'peak_parent_rss_kib': peak_parent, 'peak_children_rss_sum_kib': peak_children,
                'peak_children_pss_sum_kib': peak_pss,
                'network_unchanged': network_unchanged and network_state() == network_before,
                'shutdown_seconds': teardown, 'shutdown_action': quit_key,
                'survivors': survivors, 'exit_code': process.returncode,
                'samples': samples, 'command': command}
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        os.close(master)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--template', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--workers', type=int, nargs='+', default=[64, 500])
    parser.add_argument('--actions', nargs='+', default=['q', 'CTRL_C', 'TERM', 'HUP'])
    parser.add_argument('--openvpn', default='/usr/sbin/openvpn')
    parser.add_argument('--port', type=int, default=11999)
    args = parser.parse_args()
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    resource.setrlimit(resource.RLIMIT_NOFILE, (min(hard, max(soft, 8192)), hard))
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(('127.0.0.1', args.port))
    stop = threading.Event()
    thread = threading.Thread(target=drain, args=(sock, stop))
    thread.start()
    report = {'machine': platform.platform(), 'cpu_count': os.cpu_count(),
              'meminfo': Path('/proc/meminfo').read_text(), 'results': [],
              'limit': 'Idle TLS negotiation only; summed RSS double-counts shared pages.'}
    args.output.mkdir(parents=True, exist_ok=True)
    try:
        for workers in args.workers:
            for action in args.actions:
                result = benchmark(args, workers, action)
                report['results'].append(result)
                args.output.joinpath('results.json').write_text(json.dumps(report, indent=2))
                print({key: value for key, value in result.items() if key not in ['samples', 'command']}, flush=True)
    finally:
        stop.set()
        thread.join()
        sock.close()
    return any(row['survivors'] or row['exit_code'] != 0 or not row['network_unchanged']
               or row['peak_live_children'] != row['workers_requested'] for row in report['results'])


if __name__ == '__main__':
    raise SystemExit(main())

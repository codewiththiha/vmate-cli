#!/usr/bin/env python3
"""Exercise real Linux window identity and right-side controls under Xvfb."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def output(*arguments):
    return subprocess.check_output(arguments, text=True, timeout=15)


def expect(window, property_name, token):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        text = output('xprop', '-id', window, property_name)
        if token in text:
            return text
        time.sleep(0.02)
    raise AssertionError(f'{property_name} never contained {token}')


def main():
    binary = str(Path(sys.argv[1]).resolve())
    manager = subprocess.Popen(['openbox'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    with tempfile.TemporaryDirectory(prefix='vmate-native-') as profile:
        env = os.environ | {'XDG_CONFIG_HOME': profile, 'SLINT_BACKEND': 'winit-software'}
        app = subprocess.Popen([binary], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            window = output('xdotool', 'search', '--sync', '--onlyvisible', '--class', '^vmate$').splitlines()[0]
            identity = output('xprop', '-id', window, 'WM_CLASS')
            assert '"vmate", "vmate"' in identity, identity
            output('xdotool', 'mousemove', '--window', window, '1045', '60', 'click', '1')
            expect(window, '_NET_WM_STATE', '_NET_WM_STATE_HIDDEN')
            output('xdotool', 'windowmap', window, 'windowactivate', '--sync', window,
                   'mousemove', '--window', window, '1090', '60', 'click', '1')
            expect(window, '_NET_WM_STATE', '_NET_WM_STATE_MAXIMIZED_HORZ')
            geometry = dict(line.split('=', 1) for line in output(
                'xdotool', 'getwindowgeometry', '--shell', window).splitlines())
            right = str(int(geometry['WIDTH']) - 23)
            output('xdotool', 'mousemove', '--window', window, right, '60', 'click', '1')
            stdout, stderr = app.communicate(timeout=15)
            assert app.returncode == 0, stderr.decode(errors='replace')
            print(json.dumps({'window_class': 'vmate', 'taskbar_identity': 'passed',
                              'minimize': 'passed', 'maximize': 'passed',
                              'close_exit_code': app.returncode,
                              'network_operations': 'none requested'}))
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()
            manager.terminate()
            manager.wait(timeout=5)


if __name__ == '__main__':
    main()

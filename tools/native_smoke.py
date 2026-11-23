#!/usr/bin/env python3
"""Exercise native Linux framing, pointer input and close behavior under Xvfb."""
import json
import os
import re
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


def pointer_input(window, profile):
    time.sleep(1)
    baseline = output('xdotool', 'getwindowgeometry', '--shell', window)
    output('xdotool', 'mousemove', '--window', window, '500', '100', 'mousedown', '1',
           'mousemove', '--window', window, '540', '130', 'mouseup', '1')
    assert output('xdotool', 'getwindowgeometry', '--shell', window) == baseline
    output('xdotool', 'mousemove', '--window', window, '70', '326', 'click', '1')
    time.sleep(0.2)
    output('xdotool', 'mousemove', '--window', window, '450', '122', 'click', '1',
           'key', 'Down', 'Down', 'Return')
    output('xdotool', 'mousemove', '--window', window, '450', '372', 'click', '1',
           'key', 'ctrl+a', 'type', '--clearmodifiers', 'Pointer regression')
    output('xdotool', 'mousemove', '--window', window, '260', '722', 'click', '1')
    path = profile / 'vmate-cli/desktop.json'
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if path.exists() and any(p['name'] == 'Pointer regression'
                                 for p in json.loads(path.read_text())['presets']):
            assert not json.loads(path.read_text())['tray_enabled']
            assert json.loads(path.read_text())['appearance'] == 'dark'
            return
        time.sleep(0.05)
    raise AssertionError('Real sidebar, text entry and Save must persist the typed preset')


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
            pointer_input(window, Path(profile))
            output('xdotool', 'windowminimize', window)
            expect(window, '_NET_WM_STATE', '_NET_WM_STATE_HIDDEN')
            output('xdotool', 'windowmap', window, 'windowactivate', '--sync', window)
            geometry = output('xwininfo', '-id', window)
            x = int(re.search(r'Absolute upper-left X:\s+(\d+)', geometry)[1])
            y = int(re.search(r'Absolute upper-left Y:\s+(\d+)', geometry)[1])
            output('xdotool', 'mousemove', str(x + 500), str(y - 10),
                   'click', '--repeat', '2', '--delay', '120', '1')
            expect(window, '_NET_WM_STATE', '_NET_WM_STATE_MAXIMIZED_HORZ')
            output('xdotool', 'key', 'alt+F4')
            stdout, stderr = app.communicate(timeout=15)
            assert app.returncode == 0, stderr.decode(errors='replace')
            print(json.dumps({'window_class': 'vmate', 'taskbar_identity': 'passed',
                              'native_frame': 'passed', 'pointer_input': 'passed',
                              'theme_selection_persisted': 'passed',
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

#!/usr/bin/env python3
"""Exercise a real tray/window lifecycle with isolated NetworkManager commands."""
import asyncio
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
from dbus_next.aio import MessageBus
from tray_smoke import Watcher, close_window, menu_action, visible, wait_for

UUID = '01234567-89ab-cdef-0123-456789abcdef'


def fixture(root):
    binary = root / 'nmcli'
    binary.write_text('''#!/usr/bin/env python3
import os
from pathlib import Path
import sys
root = Path(os.environ['NM_FIXTURE'])
args = sys.argv[1:]
if 'import' in args:
    print(os.environ['NM_UUID'])
elif 'connection.type' in args:
    print('vpn')
elif '--active' in args:
    if (root / 'active').exists(): print(os.environ['NM_UUID'])
elif 'up' in args:
    (root / 'active').touch()
elif 'down' in args:
    (root / 'active').unlink(missing_ok=True)
''')
    binary.chmod(0o755)
    config = root / 'vmate-cli'
    config.mkdir()
    settings = config / 'desktop.json'
    settings.write_text(json.dumps({'tray_enabled': True, 'country_lookup': False}))
    return os.environ | {'XDG_CONFIG_HOME': str(root), 'VMATE_DB': str(root / 'history.db'),
                         'SLINT_BACKEND': 'winit-software', 'NM_FIXTURE': str(root),
                         'NM_UUID': UUID, 'PATH': f'{root}:{os.environ["PATH"]}'}


def seed(root):
    database = root / 'history.db'
    if not database.exists():
        return False
    with sqlite3.connect(database) as connection:
        if not connection.execute("SELECT 1 FROM sqlite_master WHERE name='configs'").fetchone():
            return False
        relay = root / 'relay.ovpn'
        relay.write_text('client\nremote example.invalid 1194\n')
        connection.execute("INSERT OR IGNORE INTO configs(path,path_sha256,status,country) VALUES(?,?,'success','JP')",
                           (str(relay), 'fixture'))
    return True


async def export_connected(root):
    window = visible()[0]
    subprocess.run(['xdotool', 'mousemove', '--window', window, '70', '170', 'click', '1'], check=True)
    await asyncio.sleep(0.5)
    subprocess.run(['xdotool', 'mousemove', '--window', window, '220', '146', 'sleep', '0.1', 'mousedown', '1', 'sleep', '0.1', 'mouseup', '1'], check=True)
    await asyncio.sleep(0.5)
    subprocess.run(['xdotool', 'mousemove', '--window', window, '340', '718', 'sleep', '0.1', 'mousedown', '1', 'sleep', '0.1', 'mouseup', '1'], check=True)
    await asyncio.sleep(0.5)
    destination = root / 'exports'
    destination.mkdir()
    subprocess.run(['xdotool', 'mousemove', '--window', window, '500', '250', 'click', '1',
                    'key', 'ctrl+a', 'type', '--clearmodifiers', str(destination)], check=True)
    subprocess.run(['xdotool', 'mousemove', '--window', window, '364', '425', 'click', '1'], check=True)
    await asyncio.sleep(0.5)
    subprocess.run(['xdotool', 'mousemove', '--window', window, '762', '615', 'click', '1'], check=True)
    await asyncio.sleep(0.5)
    await wait_for(lambda: list(destination.glob('*.ovpn')), 'Selected export must work while connected')
    exported = list(destination.glob('*.ovpn'))
    assert len(exported) == 1 and exported[0].name == 'Saved_config_japan.ovpn', exported
    assert exported[0].read_bytes() == (root / 'relay.ovpn').read_bytes()
    assert (root / 'active').exists(), 'Export must not disconnect'


async def exercise(binary, bus, watcher, root):
    env = fixture(root)
    process = subprocess.Popen([binary], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        await wait_for(lambda: watcher.items and visible() and seed(root), 'Fixture app must initialize')
        service = watcher.items[-1]
        active = root / 'active'
        await menu_action(bus, service, 'Connect')
        await wait_for(active.exists, 'Tray Connect must activate managed fixture')
        close_window()
        await wait_for(lambda: not visible(), 'Connected window must hide')
        await asyncio.sleep(0.5)
        assert active.exists() and process.poll() is None, 'Closing must not cancel the VPN session'
        second = subprocess.run([binary], env=env, capture_output=True, timeout=10)
        assert second.returncode == 0, second.stderr.decode()
        await wait_for(visible, 'Second launch must reopen')
        assert active.exists() and len(visible()) == 1
        await menu_action(bus, service, 'Settings')
        await asyncio.sleep(0.3)
        window = visible()[0]
        subprocess.run(['xdotool', 'mousemove', '--window', window, '201', '270', 'click', '1'], check=True)
        await asyncio.sleep(0.2)
        subprocess.run(['xdotool', 'mousemove', '--window', window, '260', '722', 'click', '1'], check=True)
        settings = root / 'vmate-cli/desktop.json'
        await wait_for(lambda: json.loads(settings.read_text()).get('reduced_motion'), 'Save while connected must work')
        assert active.exists(), 'Saving preferences must not disconnect'
        await export_connected(root)
        close_window()
        await wait_for(lambda: not visible(), 'Saved enabled tray mode must stay hidden')
        await menu_action(bus, service, 'Disconnect')
        await wait_for(lambda: not active.exists(), 'Tray Disconnect must tear down')
        await asyncio.sleep(0.3)
        assert process.poll() is None and not visible(), 'Disconnect must not reopen or quit'
        await menu_action(bus, service, 'Connect')
        await wait_for(active.exists, 'Tray must reconnect without reopening')
        await menu_action(bus, service, 'Quit vmate')
        await wait_for(lambda: process.poll() is not None, 'Quit must finish teardown')
        assert process.returncode == 0, process.stderr.read().decode()
        assert not active.exists(), 'Quit must disconnect before exiting'
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=20)
        process.stderr.close()


async def main():
    bus = await MessageBus().connect()
    watcher = Watcher()
    bus.export('/StatusNotifierWatcher', watcher)
    await bus.request_name('org.kde.StatusNotifierWatcher')
    manager = subprocess.Popen(['openbox'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        with tempfile.TemporaryDirectory(prefix='vmate-session-') as directory:
            await exercise(str(Path(sys.argv[1]).resolve()), bus, watcher, Path(directory))
        print(json.dumps({'connected_close_keeps_session': 'passed', 'save_while_connected': 'passed', 'selected_export_while_connected': 'passed',
                          'second_launch_keeps_session': 'passed', 'tray_disconnect_connect_quit': 'passed',
                          'networkmanager': 'isolated command fixture', 'real_vpn_tested': False}))
    finally:
        bus.disconnect()
        manager.terminate()
        manager.wait(timeout=5)


if __name__ == '__main__':
    asyncio.run(main())

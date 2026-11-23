#!/usr/bin/env python3
"""Exercise real app tray lifecycle against a disposable D-Bus test host."""
import asyncio
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from dbus_next import Message, MessageType, Variant
from dbus_next.aio import MessageBus
from dbus_next.service import ServiceInterface, method, dbus_property
from dbus_next.constants import PropertyAccess


class Watcher(ServiceInterface):
    def __init__(self):
        super().__init__('org.kde.StatusNotifierWatcher')
        self.items = []

    @method()
    def RegisterStatusNotifierItem(self, service: 's'):
        self.items.append(service)

    @dbus_property(access=PropertyAccess.READ)
    def IsStatusNotifierHostRegistered(self) -> 'b':
        return True

    @dbus_property(access=PropertyAccess.READ)
    def RegisteredStatusNotifierItems(self) -> 'as':
        return self.items

    @dbus_property(access=PropertyAccess.READ)
    def ProtocolVersion(self) -> 'i':
        return 0


async def wait_for(predicate, message):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if predicate():
            return
        await asyncio.sleep(0.05)
    raise AssertionError(message)


def visible():
    result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--class', '^vmate$'],
                            text=True, capture_output=True, check=False)
    return result.stdout.splitlines()


async def request(bus, destination, path, interface, member, signature='', body=None):
    reply = await bus.call(Message(destination=destination, path=path, interface=interface,
                                   member=member, signature=signature, body=body or []))
    assert reply.message_type != MessageType.ERROR, reply.body
    return reply.body


async def menu_action(bus, service, label):
    body = await request(bus, service, '/MenuBar', 'com.canonical.dbusmenu',
                         'GetLayout', 'iias', [0, -1, []])
    pending = [body[1]]
    while pending:
        identifier, properties, children = pending.pop()
        if properties.get('label', Variant('s', '')).value == label:
            await request(bus, service, '/MenuBar', 'com.canonical.dbusmenu', 'Event',
                          'isvu', [identifier, 'clicked', Variant('i', 0), 0])
            return
        pending.extend(child.value for child in children)
    raise AssertionError(f'Missing tray action: {label}')


def close_window():
    window = visible()[0]
    subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'key', 'alt+F4'], check=True)


async def exercise(binary, bus, watcher, profile, mode):
    config = profile / 'vmate-cli'
    config.mkdir()
    (config / 'desktop.json').write_text(json.dumps({'tray_enabled': mode != 'default'}))
    watcher.items.clear()
    env = os.environ | {'XDG_CONFIG_HOME': str(profile), 'SLINT_BACKEND': 'winit-software'}
    process = subprocess.Popen([binary], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        await wait_for(visible, 'App must open')
        if mode in ('default', 'unavailable'):
            await asyncio.sleep(0.5)
            assert not watcher.items, 'Tray must be opt-in'
            close_window()
        else:
            await wait_for(lambda: watcher.items, 'Tray must register with the test host')
            service = watcher.items[-1]
            await asyncio.sleep(0.2)
            close_window()
            await wait_for(lambda: not visible(), 'Close must hide to tray')
            assert process.poll() is None, 'Hidden app must remain alive'
            await request(bus, service, '/StatusNotifierItem', 'org.kde.StatusNotifierItem',
                          'Activate', 'ii', [0, 0])
            await wait_for(visible, 'Tray activation must reopen window')
            if mode == 'quit':
                close_window()
                await menu_action(bus, service, 'Quit vmate')
            elif mode == 'disable':
                await menu_action(bus, service, 'Settings')
                await asyncio.sleep(0.2)
                window = visible()[0]
                subprocess.run(['xdotool', 'mousemove', '--window', window, '201', '619',
                                'click', '1', 'mousemove', '--window', window, '260', '722',
                                'click', '1'], check=True)
                await wait_for(lambda: not json.loads((config / 'desktop.json').read_text())['tray_enabled'],
                               'Checkbox and Save must persist disabled tray')
                await asyncio.sleep(0.3)
                reply = await bus.call(Message(destination='org.freedesktop.DBus',
                    path='/org/freedesktop/DBus', interface='org.freedesktop.DBus',
                    member='NameHasOwner', signature='s', body=[service]))
                assert not reply.body[0], 'Disabling must unregister the tray'
                close_window()
            else:
                close_window()
                await wait_for(lambda: not visible(), 'App must hide before host loss')
                await bus.release_name('org.kde.StatusNotifierWatcher')
                await wait_for(visible, 'Losing the host must leave a reachable window')
                await asyncio.sleep(0.2)
                close_window()
        await wait_for(lambda: process.poll() is not None, 'Quit must finish teardown')
        assert process.returncode == 0, process.stderr.read().decode()
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
        for mode in ('default', 'quit', 'disable', 'host-loss', 'unavailable'):
            with tempfile.TemporaryDirectory(prefix='vmate-tray-') as directory:
                await exercise(str(Path(sys.argv[1]).resolve()), bus, watcher, Path(directory), mode)
        print(json.dumps({'tray_default_off': 'passed', 'close_reopen': 'passed',
                          'tray_quit': 'passed', 'disable_unregister': 'passed',
                          'host_loss_recovery': 'passed', 'unavailable_host_close': 'passed', 'host': 'disposable D-Bus fixture',
                          'network_operations': 'none requested'}))
    finally:
        bus.disconnect()
        manager.terminate()
        manager.wait(timeout=5)


if __name__ == '__main__':
    asyncio.run(main())

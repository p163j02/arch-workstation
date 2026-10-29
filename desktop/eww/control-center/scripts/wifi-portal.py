#!/usr/bin/env python3
"""Open captive portals once per active Wi-Fi connection, outside Eww's lifecycle."""
import json
import logging
import os
from pathlib import Path
import signal

import gi

gi.require_version('GLibUnix', '2.0')
from gi.repository import Gio, GLib, GLibUnix

NM = 'org.freedesktop.NetworkManager'
ROOT = '/org/freedesktop/NetworkManager'
DEVICE = NM + '.Device'
ACTIVE = NM + '.Connection.Active'
PORTAL, FULL = 2, 4
# HTTP is intentional: the gateway must be able to redirect to its login page.
# Never launch a URL supplied by an untrusted SSID or a DBus signal.
PORTAL_URL = 'http://neverssl.com/'
LOG = logging.getLogger('wifi-portal')


def eligible(properties, connection, primary, connectivity):
    """Only an activated, primary/default Wi-Fi route can launch the browser."""
    if properties.get('DeviceType') != 2 or properties.get('State') != 100:
        return False
    if properties.get('ActiveConnection', '/') != primary or primary == '/':
        return False
    if not (connection.get('Default') or connection.get('Default6')):
        return False
    checks = [properties.get('Ip4Connectivity'), properties.get('Ip6Connectivity')]
    # A working IP family is enough for Internet access. Do not open for outages,
    # limited connectivity, Ethernet, VPNs, or a secondary Wi-Fi connection.
    if FULL in checks or connectivity == FULL:
        return False
    return PORTAL in checks or connectivity == PORTAL


class OncePerConnection:
    """Runtime-only deduplication survives service restarts, not disconnections."""
    def __init__(self, path):
        self.path = path
        try:
            data = json.loads(path.read_text())
            self.opened = set(data) if isinstance(data, list) and all(isinstance(x, str) for x in data) else set()
        except (OSError, ValueError):
            self.opened = set()

    def save(self):
        self.path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        tmp = self.path.with_suffix('.tmp')
        tmp.write_text(json.dumps(sorted(self.opened)))
        tmp.chmod(0o600)
        tmp.replace(self.path)

    def update(self, active, portals, launch):
        previous = self.opened.copy()
        self.opened.intersection_update(active)
        for key in sorted((set(portals) & set(active)) - self.opened):
            # Mark only after successfully spawning the opener. It receives a
            # fixed argv; there is no shell or credential handling here.
            launch()
            self.opened.add(key)
        if previous != self.opened:
            self.save()


class Monitor:
    def __init__(self):
        runtime = Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}'))
        self.once = OncePerConnection(runtime / 'waybar-control-center' / 'wifi-portal-opened.json')
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        self.loop = GLib.MainLoop()
        self.pending = None
        self.bus.signal_subscribe(NM, 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
                                  None, None, Gio.DBusSignalFlags.NONE, self.changed)
        self.bus.signal_subscribe('org.freedesktop.DBus', 'org.freedesktop.DBus', 'NameOwnerChanged',
                                  '/org/freedesktop/DBus', NM, Gio.DBusSignalFlags.NONE, self.owner_changed)

    def properties(self, path, interface):
        return self.bus.call_sync(NM, path, 'org.freedesktop.DBus.Properties', 'GetAll',
                                  GLib.Variant('(s)', (interface,)), None,
                                  Gio.DBusCallFlags.NONE, 3000, None).unpack()[0]

    def snapshot(self):
        manager = self.properties(ROOT, NM)
        active, portals = set(), set()
        for path in manager.get('Devices', []):
            device = self.properties(path, DEVICE)
            connection_path = device.get('ActiveConnection', '/')
            if device.get('DeviceType') != 2 or device.get('State') != 100 or connection_path == '/':
                continue
            # NM assigns a fresh object path on reconnect, even for the same SSID.
            key = path + ':' + connection_path
            active.add(key)
            connection = self.properties(connection_path, ACTIVE)
            if eligible(device, connection, manager.get('PrimaryConnection', '/'), manager.get('Connectivity')):
                portals.add(key)
        return active, portals

    def launch(self):
        if not (os.environ.get('WAYLAND_DISPLAY') or os.environ.get('DISPLAY')):
            raise OSError('La sesión gráfica aún no está disponible')
        process = Gio.Subprocess.new(['/usr/bin/xdg-open', PORTAL_URL],
                                     Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE)
        def finished(child, result):
            try:
                child.wait_check_finish(result)
                LOG.info('Formulario del portal solicitado al navegador predeterminado.')
            except GLib.Error:
                LOG.warning('El navegador no pudo abrirse; abre http://neverssl.com/ manualmente.')
        process.wait_check_async(None, finished)

    def refresh(self):
        self.pending = None
        try:
            active, portals = self.snapshot()
            self.once.update(active, portals, self.launch)
        except (GLib.Error, OSError, ValueError):
            # A failed/partial snapshot must not reset deduplication.
            LOG.warning('No se pudo comprobar el portal; se reintentará automáticamente.')
        return GLib.SOURCE_REMOVE

    def changed(self, bus, sender, path, interface, name, parameters):
        changed_interface, changed, invalidated = parameters.unpack()
        relevant = {
            NM: {'Connectivity', 'PrimaryConnection', 'Devices', 'ActiveConnections'},
            DEVICE: {'State', 'ActiveConnection', 'Ip4Connectivity', 'Ip6Connectivity'},
            ACTIVE: {'State', 'Default', 'Default6'},
        }
        if relevant.get(changed_interface, set()) & (set(changed) | set(invalidated)):
            self.schedule()

    def owner_changed(self, *args):
        # Object paths may be reused after a NetworkManager daemon restart.
        self.once.opened.clear()
        self.once.save()
        self.schedule()

    def schedule(self):
        if self.pending is None:
            self.pending = GLib.timeout_add(750, self.refresh)

    def recovery(self):
        self.schedule()
        return GLib.SOURCE_CONTINUE

    def run(self):
        for sig in (signal.SIGTERM, signal.SIGINT):
            GLibUnix.signal_add(GLib.PRIORITY_DEFAULT, sig, lambda: (self.loop.quit(), False)[1])
        self.schedule()
        # Read-only fallback; no Wi-Fi scans or forced HTTP checks.
        GLib.timeout_add_seconds(30, self.recovery)
        LOG.info('Detección de portales Wi-Fi activa (eventos NetworkManager).')
        self.loop.run()


if __name__ == '__main__':
    logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')
    Monitor().run()

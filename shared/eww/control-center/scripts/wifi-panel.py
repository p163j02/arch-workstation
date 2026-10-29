#!/usr/bin/env python3
"""Native GTK3 Wi-Fi popup (layer-shell) with in-memory secret handling."""
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import uuid
from concurrent.futures import ThreadPoolExecutor

import gi

gi.require_version('Gtk', '3.0')
gi.require_version('Gdk', '3.0')
gi.require_version('GtkLayerShell', '0.1')
gi.require_version('NM', '1.0')
from gi.repository import Gdk, Gio, GLib, Gtk, GtkLayerShell, NM

ROOT = Path(__file__).resolve().parents[1]
POPUP_MANAGER = ROOT / 'scripts' / 'popup-manager.py'
sys.path.insert(0, str(ROOT / 'scripts'))
import backend

NM_DEST = 'org.freedesktop.NetworkManager'
NM_ROOT = '/org/freedesktop/NetworkManager'
NM_DEV_IFACE = 'org.freedesktop.NetworkManager.Device.Wireless'
NM_DEVICE_IFACE = 'org.freedesktop.NetworkManager.Device'
NM_AP_IFACE = 'org.freedesktop.NetworkManager.AccessPoint'
NM_ACTIVE_IFACE = 'org.freedesktop.NetworkManager.Connection.Active'
NM_SETTINGS = 'org.freedesktop.NetworkManager.Settings'
NM_SETTINGS_CONN = 'org.freedesktop.NetworkManager.Settings.Connection'
DBUS_PROPS = 'org.freedesktop.DBus.Properties'
PID_FILE = backend.RUN / 'wifi-panel.pid'

PSK_RE = re.compile(r'^[0-9a-fA-F]{64}$')
CONNECT_TIMEOUT = 45


class ConnectionFailure(RuntimeError):
    """User-safe connection failure; never contains the submitted secret."""


def activation_failure(bus, device_path, prop_getter):
    try:
        _state, reason = prop_getter(bus, device_path, NM_DEVICE_IFACE, 'StateReason')
        reasons = NM.DeviceStateReason
        if int(reason) in {int(reasons.NO_SECRETS), int(reasons.SUPPLICANT_DISCONNECT),
                          int(reasons.SUPPLICANT_CONFIG_FAILED), int(reasons.SUPPLICANT_FAILED),
                          int(reasons.SUPPLICANT_TIMEOUT)}:
            return 'No se pudo autenticar. La contraseña puede ser incorrecta; revísala e intenta de nuevo.'
        if int(reason) in {int(reasons.IP_CONFIG_UNAVAILABLE), int(reasons.IP_CONFIG_EXPIRED)}:
            return 'No se pudo obtener una dirección IP. Revisa el router e intenta de nuevo.'
    except Exception:
        pass
    return generic_connect_error()

CSS = b'''
* { all: unset; font-family: "JetBrainsMono Nerd Font"; font-size: 13px; color: #c0caf5; }
window { background: transparent; }
.cc-panel { background: rgba(26,27,38,.96); border: 1px solid rgba(122,162,247,.4); border-radius: 15px; padding: 20px; }
.cc-header { padding-bottom: 12px; border-bottom: 1px solid #292e42; }
.cc-title { font-size: 16px; font-weight: bold; }
.cc-icon { font-size: 24px; color: #7aa2f7; }
.cc-section { color: #9aa5ce; font-size: 12px; margin-top: 2px; }
.cc-accent { color: #7aa2f7; }
.cc-secondary { color: #9aa5ce; font-size: 12px; }
.cc-current { background: rgba(41,46,66,.42); border-radius: 10px; padding: 12px; }
.cc-toggle { font-size: 26px; padding: 0 4px; color: #565f89; }
.cc-toggle.on { color: #7aa2f7; }
.cc-close, .cc-gear { color: #9aa5ce; padding: 4px 7px; border-radius: 8px; }
.cc-row-icon { color: #7aa2f7; font-size: 20px; }
.cc-network.selected { background: rgba(122,162,247,.14); color: #7aa2f7; border-radius: 8px; }
.cc-password { background: rgba(41,46,66,.42); border-radius: 10px; padding: 10px; }
.cc-error { color: #e0af68; font-size: 11px; }
.cc-info { color: #7dcfff; font-size: 11px; }
.cc-spinner { color: #7aa2f7; font-size: 18px; min-width: 22px; }
.cc-scan { color: #7aa2f7; border: 1px solid #414868; }
button { padding: 7px 10px; border-radius: 8px; }
button:hover { background: #292e42; }
button:active { background: #3d59a1; }
button:focus, entry:focus { border: 1px solid #7aa2f7; }
entry { background: #24283b; border: 1px solid #414868; border-radius: 8px; padding: 8px; color: #c0caf5; }
scrollbar slider { background: #414868; border-radius: 4px; }
'''


def classify_security(privacy, wpa_flags, rsn_flags):
    """Map AP flags to user-visible auth type."""
    sec = getattr(NM, '80211ApSecurityFlags')
    psk_mask = int(sec.KEY_MGMT_PSK) | int(sec.KEY_MGMT_SAE)
    enterprise_mask = int(sec.KEY_MGMT_802_1X) | int(sec.KEY_MGMT_EAP_SUITE_B_192)
    auth = int(wpa_flags) | int(rsn_flags)
    if not privacy and auth == 0:
        return 'open'
    if auth & enterprise_mask:
        return 'enterprise'
    if auth & psk_mask:
        return 'psk'
    if privacy and auth == 0:
        return 'wep'
    return 'unknown'


def choose_psk_key_mgmts(wpa_flags, rsn_flags):
    sec = getattr(NM, '80211ApSecurityFlags')
    auth = int(wpa_flags) | int(rsn_flags)
    has_sae = bool(auth & int(sec.KEY_MGMT_SAE))
    has_psk = bool(auth & int(sec.KEY_MGMT_PSK))
    if has_sae and has_psk:
        return ['sae', 'wpa-psk']
    if has_sae:
        return ['sae', 'wpa-psk']
    return ['wpa-psk']


def valid_psk(secret):
    return (8 <= len(secret) <= 63) or bool(PSK_RE.fullmatch(secret))


def build_psk_settings(ssid, password, key_mgmt):
    """Variant map for AddAndActivateConnection. Secret never leaves memory."""
    return {
        'connection': {
            'id': GLib.Variant('s', f'{ssid} (inline)'),
            'type': GLib.Variant('s', '802-11-wireless'),
            'uuid': GLib.Variant('s', str(uuid.uuid4())),
            'autoconnect': GLib.Variant('b', True),
        },
        '802-11-wireless': {
            'ssid': GLib.Variant('ay', ssid.encode('utf-8')),
            'mode': GLib.Variant('s', 'infrastructure'),
        },
        '802-11-wireless-security': {
            'key-mgmt': GLib.Variant('s', key_mgmt),
            'psk': GLib.Variant('s', password),
        },
        'ipv4': {'method': GLib.Variant('s', 'auto')},
        'ipv6': {'method': GLib.Variant('s', 'auto')},
    }


def build_open_settings(ssid):
    return {
        'connection': {
            'id': GLib.Variant('s', f'{ssid} (open inline)'),
            'type': GLib.Variant('s', '802-11-wireless'),
            'uuid': GLib.Variant('s', str(uuid.uuid4())),
            'autoconnect': GLib.Variant('b', True),
        },
        '802-11-wireless': {
            'ssid': GLib.Variant('ay', ssid.encode('utf-8')),
            'mode': GLib.Variant('s', 'infrastructure'),
        },
        'ipv4': {'method': GLib.Variant('s', 'auto')},
        'ipv6': {'method': GLib.Variant('s', 'auto')},
    }


def generic_connect_error():
    return 'No se pudo completar la conexión Wi-Fi. Verifica la contraseña o vuelve a intentar.'


def profile_iface_matches(bound_iface, iface):
    return bound_iface in ('', None, '--', iface)


def wait_for_activation(bus, active_path, timeout=CONNECT_TIMEOUT, interval=0.2, prop_getter=None, device_path=None):
    prop_getter = get_prop if prop_getter is None else prop_getter
    state_enum = getattr(NM, 'ActiveConnectionState')
    activated = int(state_enum.ACTIVATED)
    failed = {int(state_enum.DEACTIVATED), int(state_enum.DEACTIVATING)}
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = int(prop_getter(bus, active_path, NM_ACTIVE_IFACE, 'State'))
        if state == activated:
            return
        if state in failed:
            raise ConnectionFailure(activation_failure(bus, device_path, prop_getter))
        time.sleep(interval)
    raise ConnectionFailure('La conexión Wi-Fi tardó demasiado. Revisa la contraseña y la señal, y vuelve a intentar.')


def nm_device_path(bus, iface):
    return bus.call_sync(
        NM_DEST, NM_ROOT, NM_DEST, 'GetDeviceByIpIface',
        GLib.Variant('(s)', (iface,)), GLib.VariantType.new('(o)'),
        Gio.DBusCallFlags.NONE, 8000, None,
    ).unpack()[0]


def get_prop(bus, path, interface, name):
    return bus.call_sync(
        NM_DEST, path, DBUS_PROPS, 'Get',
        GLib.Variant('(ss)', (interface, name)), None,
        Gio.DBusCallFlags.NONE, 8000, None,
    ).unpack()[0]


def find_ap_path(bus, device_path, bssid):
    access_points = get_prop(bus, device_path, NM_DEV_IFACE, 'AccessPoints')
    wanted = bssid.upper()
    for ap in access_points:
        if str(get_prop(bus, ap, NM_AP_IFACE, 'HwAddress')).upper() == wanted:
            return ap
    raise RuntimeError('La red ya no está disponible. Vuelve a escanear.')


class WifiPanel:
    def __init__(self, x, y, width, maxheight, screen, monitor):
        self.x, self.y = x, y
        self.width = width
        self.maxheight = maxheight
        self.screen_index = screen
        self.monitor_name = monitor
        self.executor = ThreadPoolExecutor(max_workers=4, thread_name_prefix='wifi-panel')
        self.client = None
        self.iface = None
        self.initializing = True
        self.snapshot_ready = False
        self.state = dict(backend.DEFAULTS['wifi'], busy=False, error='')
        self.saved_ssids = set()
        self.saved_profiles = {}
        self.ap_meta = {}
        self.pending = None
        self.local_error = ''
        self.local_info = ''
        self.local_busy = False
        self.local_progress = ''
        self.refresh_error = ''
        self.refresh_future = None
        self.network_signature = None
        self.running = True
        self.spinner_frame = 0

        self.window = Gtk.Window(type=Gtk.WindowType.TOPLEVEL)
        self.window.set_decorated(False)
        self.window.set_resizable(False)
        self.window.connect('delete-event', self.on_close)
        self.window.connect('key-press-event', self.on_key)

        GtkLayerShell.init_for_window(self.window)
        GtkLayerShell.set_namespace(self.window, 'waybar-control-center')
        GtkLayerShell.set_layer(self.window, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_anchor(self.window, GtkLayerShell.Edge.TOP, True)
        GtkLayerShell.set_anchor(self.window, GtkLayerShell.Edge.LEFT, True)
        GtkLayerShell.set_anchor(self.window, GtkLayerShell.Edge.RIGHT, False)
        GtkLayerShell.set_anchor(self.window, GtkLayerShell.Edge.BOTTOM, False)
        GtkLayerShell.set_margin(self.window, GtkLayerShell.Edge.LEFT, x)
        GtkLayerShell.set_margin(self.window, GtkLayerShell.Edge.TOP, y)
        GtkLayerShell.set_keyboard_mode(self.window, GtkLayerShell.KeyboardMode.ON_DEMAND)
        GtkLayerShell.set_exclusive_zone(self.window, 0)

        display = Gdk.Display.get_default()
        if display is not None and 0 <= self.screen_index < display.get_n_monitors():
            GtkLayerShell.set_monitor(self.window, display.get_monitor(self.screen_index))

        self.window.set_size_request(width, maxheight)

        provider = Gtk.CssProvider()
        provider.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

        self.theme_provider = Gtk.CssProvider()
        self.theme_signature = None
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), self.theme_provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1
        )
        self.refresh_theme()
        GLib.timeout_add(500, self.refresh_theme)

        self.container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        self.container.get_style_context().add_class('cc-panel')
        self.container.get_style_context().add_class('wifi')

        self.build_ui()
        self.scroller = Gtk.ScrolledWindow()
        self.scroller.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        if hasattr(self.scroller, 'set_max_content_height'):
            self.scroller.set_max_content_height(maxheight)
        self.scroller.set_propagate_natural_height(True)
        self.scroller.add(self.container)
        self.window.add(self.scroller)

        self.window.show_all()
        self.render()
        # Map the window first; neither nmcli nor synchronous NM initialization
        # belongs on the first-frame path.
        GLib.idle_add(self.initialize_network)

        GLib.timeout_add(1000, self.poll)
        GLib.timeout_add(100, self.animate_spinner)

    def refresh_theme(self):
        if not self.running:
            return False
        path = ROOT / 'wallpaper-studio.scss'
        try:
            stat = path.stat()
            signature = (stat.st_ino, stat.st_mtime_ns, stat.st_size)
            if signature != self.theme_signature:
                provider = Gtk.CssProvider()
                provider.load_from_data(path.read_bytes())
                screen = Gdk.Screen.get_default()
                if screen is not None:
                    Gtk.StyleContext.add_provider_for_screen(screen, provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
                    Gtk.StyleContext.remove_provider_for_screen(screen, self.theme_provider)
                self.theme_provider = provider
                self.theme_signature = signature
        except (OSError, GLib.Error):
            pass  # Keep the last valid palette during an incomplete update.
        return True

    def build_ui(self):
        header = Gtk.Box(spacing=12)
        header.get_style_context().add_class('cc-header')
        icon = Gtk.Label(label='󰤨')
        icon.get_style_context().add_class('cc-icon')
        title = Gtk.Label(label='Wi-Fi')
        title.set_xalign(0)
        title.set_hexpand(True)
        title.get_style_context().add_class('cc-title')
        gear_btn = Gtk.Button(label='󰒓')
        gear_btn.get_style_context().add_class('cc-gear')
        gear_btn.set_tooltip_text('Elegir gestor de Wi-Fi')
        gear_btn.connect('clicked', self.open_editor)
        close_btn = Gtk.Button(label='󰅖')
        close_btn.get_style_context().add_class('cc-close')
        close_btn.connect('clicked', lambda *_: self.request_popup_close())
        header.pack_start(icon, False, False, 0)
        header.pack_start(title, True, True, 0)
        header.pack_start(gear_btn, False, False, 0)
        header.pack_start(close_btn, False, False, 0)
        self.container.pack_start(header, False, False, 0)

        toggle_row = Gtk.Box(spacing=12)
        label = Gtk.Label(label='Wi-Fi')
        label.set_hexpand(True)
        label.set_xalign(0)
        self.toggle_btn = Gtk.Button()
        self.toggle_btn.connect('clicked', self.on_toggle)
        toggle_row.pack_start(label, True, True, 0)
        toggle_row.pack_start(self.toggle_btn, False, False, 0)
        self.container.pack_start(toggle_row, False, False, 0)

        self.current_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        self.current_box.get_style_context().add_class('cc-current')
        self.ssid_label = Gtk.Label()
        self.ssid_label.set_xalign(0)
        self.ssid_label.set_ellipsize(3)
        self.ssid_label.get_style_context().add_class('cc-accent')
        self.status_label = Gtk.Label()
        self.status_label.set_xalign(0)
        self.status_label.get_style_context().add_class('cc-secondary')
        self.disconnect_btn = Gtk.Button(label='Desconectar')
        self.disconnect_btn.connect('clicked', self.on_disconnect)
        self.current_box.pack_start(self.ssid_label, False, False, 0)
        self.current_box.pack_start(self.status_label, False, False, 0)
        self.current_box.pack_start(self.disconnect_btn, False, False, 0)
        self.container.pack_start(self.current_box, False, False, 0)

        section_row = Gtk.Box(spacing=8)
        section = Gtk.Label(label='Redes disponibles')
        section.set_xalign(0)
        section.set_hexpand(True)
        section.get_style_context().add_class('cc-section')
        self.scan_btn = Gtk.Button()
        self.scan_btn.get_style_context().add_class('cc-scan')
        self.scan_btn.connect('clicked', self.on_rescan)
        section_row.pack_start(section, True, True, 0)
        section_row.pack_start(self.scan_btn, False, False, 0)
        self.container.pack_start(section_row, False, False, 0)

        self.list_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.list_scroll = Gtk.ScrolledWindow()
        self.list_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        if hasattr(self.list_scroll, 'set_max_content_height'):
            self.list_scroll.set_max_content_height(190)
        self.list_scroll.set_min_content_height(190)
        self.list_scroll.add(self.list_box)
        self.container.pack_start(self.list_scroll, False, False, 0)

        self.password_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.password_box.get_style_context().add_class('cc-password')
        self.password_title = Gtk.Label(label='')
        self.password_title.set_xalign(0)
        self.password_title.set_line_wrap(True)
        self.password_title.set_max_width_chars(42)
        self.password_title.set_ellipsize(3)
        self.password_title.get_style_context().add_class('cc-accent')
        row = Gtk.Box(spacing=8)
        self.password_entry = Gtk.Entry()
        self.password_entry.set_visibility(False)
        self.password_entry.set_invisible_char('•')
        self.password_entry.set_placeholder_text('Contraseña Wi-Fi')
        self.password_entry.connect('activate', self.on_password_connect)
        self.show_btn = Gtk.Button(label='󰈉')
        self.show_btn.connect('clicked', self.on_toggle_password)
        row.pack_start(self.password_entry, True, True, 0)
        row.pack_start(self.show_btn, False, False, 0)
        actions = Gtk.Box(spacing=8)
        connect_btn = Gtk.Button(label='Conectar')
        connect_btn.connect('clicked', self.on_password_connect)
        cancel_btn = Gtk.Button(label='Cancelar')
        cancel_btn.connect('clicked', lambda *_: self.hide_password())
        actions.pack_start(connect_btn, False, False, 0)
        actions.pack_start(cancel_btn, False, False, 0)
        self.saved_btn = Gtk.Button(label='Usar guardada')
        self.saved_btn.connect('clicked', self.on_use_saved)
        actions.pack_start(self.saved_btn, False, False, 0)
        self.password_hint = Gtk.Label(label='')
        self.password_hint.set_xalign(0)
        self.password_hint.set_line_wrap(True)
        self.password_hint.set_max_width_chars(42)
        self.password_hint.get_style_context().add_class('cc-secondary')
        self.password_box.pack_start(self.password_title, False, False, 0)
        self.password_box.pack_start(row, False, False, 0)
        self.password_box.pack_start(actions, False, False, 0)
        self.password_box.pack_start(self.password_hint, False, False, 0)
        self.password_box.set_no_show_all(True)
        self.password_box.hide()
        self.container.pack_start(self.password_box, False, False, 0)
        self.container.reorder_child(self.password_box, 3)

        self.error = Gtk.Label(label='')
        self.error.set_xalign(0)
        self.error.set_line_wrap(True)
        self.error.set_selectable(False)
        self.error.get_style_context().add_class('cc-error')
        self.error.set_max_width_chars(36)
        self.feedback_box = Gtk.Box(spacing=10)
        self.loading_icon = Gtk.Label(label='◐')
        self.loading_icon.get_style_context().add_class('cc-spinner')
        self.feedback_box.pack_start(self.loading_icon, False, False, 0)
        self.feedback_box.pack_start(self.error, True, True, 0)
        self.container.pack_start(self.feedback_box, False, False, 0)
        self.container.reorder_child(self.feedback_box, 3)

    def initialize_network(self):
        if not self.running:
            return False
        NM.Client.new_async(None, self.on_client_ready, None)
        self.poll()
        return False

    def on_client_ready(self, _source, result, _data=None):
        if not self.running:
            return
        try:
            self.client = NM.Client.new_finish(result)
            self.update_initialization()
        except Exception:
            self.initializing = False
            self.refresh_error = 'No se pudo consultar NetworkManager. Cierra y vuelve a abrir el panel.'
        self.render()

    def update_initialization(self):
        if self.client is not None and self.snapshot_ready:
            self.refresh_saved_profiles()
            self.refresh_ap_meta()
            self.initializing = False

    def read_snapshot(self):
        iface = self.iface or backend.wifi_interface()
        return iface, backend.status('wifi')

    def refresh_saved_profiles(self):
        saved = set()
        profiles = {}
        for conn in self.client.get_connections() or []:
            if not isinstance(conn, NM.RemoteConnection):
                continue
            sw = conn.get_setting_wireless()
            if sw is None or sw.get_ssid() is None:
                continue
            setting_conn = conn.get_setting_connection()
            bound_iface = setting_conn.get_interface_name() if setting_conn else ''
            if not profile_iface_matches(bound_iface, self.iface):
                continue
            data = bytes(sw.get_ssid().get_data())
            name = NM.utils_ssid_to_utf8(data) if data else ''
            path = conn.get_path() if hasattr(conn, 'get_path') else ''
            if name and path:
                saved.add(name)
                profiles.setdefault(name, []).append(path)
        self.saved_ssids = saved
        self.saved_profiles = profiles

    def refresh_ap_meta(self):
        ap_meta = {}
        for dev in self.client.get_devices() or []:
            if not isinstance(dev, NM.DeviceWifi):
                continue
            if dev.get_iface() != self.iface:
                continue
            for ap in dev.get_access_points() or []:
                bssid = ap.get_bssid()
                if not bssid:
                    continue
                ap_meta[bssid.upper()] = {
                    'security': classify_security(bool(ap.get_flags() & getattr(NM, '80211ApFlags').PRIVACY), ap.get_wpa_flags(), ap.get_rsn_flags()),
                    'wpa': int(ap.get_wpa_flags()),
                    'rsn': int(ap.get_rsn_flags()),
                }
        self.ap_meta = ap_meta

    def on_toggle_password(self, *_):
        visible = self.password_entry.get_visibility()
        self.password_entry.set_visibility(not visible)
        self.show_btn.set_label('󰈈' if not visible else '󰈉')

    def on_key(self, _widget, event):
        if event.keyval == Gdk.KEY_Escape:
            self.request_popup_close()
            return True
        return False

    def on_close(self, *_):
        self.close()
        return True

    def request_popup_close(self):
        python = sys.executable if Path(sys.executable).name.startswith('python') else '/usr/bin/python3'
        try:
            subprocess.Popen([python, str(POPUP_MANAGER), 'close'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True)
        except Exception:
            self.close()

    def close(self):
        if not self.running:
            return
        self.running = False
        self.executor.shutdown(wait=False, cancel_futures=True)
        Gtk.main_quit()

    def animate_spinner(self):
        if not self.running:
            return False
        if self.initializing or self.local_busy or self.state.get('busy'):
            self.spinner_frame = (self.spinner_frame + 1) % 4
            self.loading_icon.set_text(('◐', '◓', '◑', '◒')[self.spinner_frame])
        return True

    def poll(self):
        if not self.running:
            return False
        if self.refresh_future and not self.refresh_future.done():
            return True
        self.refresh_future = self.executor.submit(self.read_snapshot)
        self.refresh_future.add_done_callback(lambda f: GLib.idle_add(self.apply_refresh_snapshot, f))
        return True

    def apply_refresh_snapshot(self, future):
        if not self.running:
            return False
        try:
            self.iface, self.state = future.result()
            self.snapshot_ready = True
            self.refresh_error = ''
            self.update_initialization()
        except Exception as e:
            self.initializing = False
            self.refresh_error = str(e)
        self.render()
        return False

    def run_task(self, fn, progress='', success='', on_error=None):
        if self.local_busy:
            return
        self.local_busy = True
        self.local_progress = progress
        self.local_info = ''
        self.local_error = ''
        self.render()
        future = self.executor.submit(fn)
        future.add_done_callback(lambda f: GLib.idle_add(self.finish_task, f, success, on_error))

    def finish_task(self, future, success, on_error):
        if not self.running:
            return False
        self.local_busy = False
        self.local_progress = ''
        handled = False
        try:
            future.result()
            self.local_error = ''
            self.local_info = success
        except Exception as e:
            if on_error is not None:
                handled = bool(on_error(e))
            if not handled:
                self.local_info = ''
                self.local_error = str(e)
        self.poll()
        self.render()
        return False

    def on_toggle(self, *_):
        if self.local_busy:
            return
        self.run_task(lambda: backend.action('wifi', 'toggle'), progress='Cambiando estado de Wi-Fi…')

    def on_rescan(self, *_):
        if self.local_busy:
            return
        self.run_task(lambda: backend.action('wifi', 'rescan'), progress='Buscando redes…', success='Lista de redes actualizada.')

    def on_disconnect(self, *_):
        if self.local_busy:
            return
        self.run_task(lambda: backend.action('wifi', 'disconnect'), progress='Desconectando…', success='Red desconectada.')

    def show_password_prompt(self, network, hint):
        self.pending = dict(network)
        self.password_title.set_text(f"Contraseña para {network['name']}")
        self.password_hint.set_text(hint)
        self.password_entry.set_text('')
        self.password_entry.set_visibility(False)
        self.show_btn.set_label('󰈉')
        self.password_box.set_no_show_all(False)
        self.password_box.show_all()
        self.saved_btn.set_visible(network['name'] in self.saved_ssids)
        self.password_entry.grab_focus()
        GLib.idle_add(self.scroll_to_password)

    def on_network(self, _btn, network):
        if self.initializing or self.local_busy or self.state.get('busy') or not self.state.get('enabled'):
            return
        if network.get('connected'):
            return
        bssid = network['id'].upper()
        security = self.ap_meta.get(bssid, {}).get('security', 'psk' if network.get('secure') else 'open')

        if not network.get('secure'):
            self.hide_password()
            self.run_task(
                lambda: self.connect_open(network['id'], network['name']),
                progress=f"Conectando a {network['name']}…",
                success=f"Conectado a {network['name']}",
            )
            return

        if security == 'psk':
            self.show_password_prompt(network, 'Introduce la contraseña o usa el perfil guardado.')
            return

        if network.get('name') in self.saved_ssids:
            self.hide_password()
            self.run_task(
                lambda: self.connect_saved_profile(network),
                progress=f"Activando perfil guardado para {network['name']}…",
                success=f"Conectado a {network['name']}",
                on_error=lambda exc: self.handle_saved_profile_error(network, security, exc),
            )
            return

        if security != 'psk':
            self.pending = None
            self.password_title.set_text(f"{network['name']} requiere configuración avanzada")
            self.password_hint.set_text('Red empresarial o WEP. Usa nm-connection-editor para completar.')
            self.password_entry.set_text('')
            self.password_box.set_no_show_all(False)
            self.password_box.show_all()
            GLib.idle_add(self.scroll_to_password)
            return

        self.show_password_prompt(network, 'WPA2/WPA3 Personal (PSK)')

    def on_use_saved(self, *_):
        if self.local_busy or not self.pending:
            return
        network = dict(self.pending)
        self.hide_password()
        self.run_task(lambda: self.connect_saved_profile(network),
            progress='Activando perfil guardado…', success='Conexión establecida.',
            on_error=lambda exc: self.handle_saved_profile_error(network, 'psk', exc))

    def scroll_to_password(self):
        adjustment = self.scroller.get_vadjustment()
        adjustment.set_value(0)
        return False

    def hide_password(self):
        self.pending = None
        self.password_entry.set_text('')
        self.password_entry.set_visibility(False)
        self.show_btn.set_label('󰈉')
        self.password_box.set_no_show_all(True)
        self.password_box.hide()

    def on_password_connect(self, *_):
        if self.local_busy or not self.pending:
            return
        secret = self.password_entry.get_text()
        if not valid_psk(secret):
            self.local_error = 'La contraseña WPA debe tener 8-63 caracteres o 64 hex.'
            self.render()
            return
        network = dict(self.pending)
        bssid = network['id'].upper()
        meta = self.ap_meta.get(bssid, {'wpa': 0, 'rsn': 0})
        key_mgmts = choose_psk_key_mgmts(meta.get('wpa', 0), meta.get('rsn', 0))
        self.hide_password()
        self.run_task(
            lambda: self.connect_with_secret(network['id'], network['name'], secret, key_mgmts),
            progress=f"Conectando a {network['name']}…",
            success=f"Conectado a {network['name']}",
            on_error=lambda exc: self.handle_password_error(network, exc),
        )

    def handle_password_error(self, network, exc):
        self.show_password_prompt(network, 'Vuelve a introducir la contraseña para intentar de nuevo.')
        self.local_error = str(exc) if isinstance(exc, ConnectionFailure) else generic_connect_error()
        self.local_info = ''
        return True

    def activate_saved(self, bus, saved_path, device_path, ap_path):
        return bus.call_sync(
            NM_DEST,
            NM_ROOT,
            NM_DEST,
            'ActivateConnection',
            GLib.Variant('(ooo)', (saved_path, device_path, ap_path)),
            GLib.VariantType.new('(o)'),
            Gio.DBusCallFlags.NONE,
            20000,
            None,
        ).unpack()[0]

    def connect_saved_profile(self, network):
        bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        device_path = nm_device_path(bus, self.iface)
        ap_path = find_ap_path(bus, device_path, network['id'])
        paths = self.saved_profiles.get(network['name'], [])
        if not paths:
            raise RuntimeError('No hay un perfil guardado compatible con esta interfaz.')
        for saved_path in paths:
            try:
                active_path = self.activate_saved(bus, saved_path, device_path, ap_path)
                wait_for_activation(bus, active_path, device_path=device_path)
                return
            except (GLib.Error, RuntimeError):
                continue
        raise RuntimeError(generic_connect_error())

    def connect_open(self, bssid, ssid):
        bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        device_path = nm_device_path(bus, self.iface)
        ap_path = find_ap_path(bus, device_path, bssid)
        settings = build_open_settings(ssid)
        try:
            _conn_path, active_path = bus.call_sync(
                NM_DEST,
                NM_ROOT,
                NM_DEST,
                'AddAndActivateConnection',
                GLib.Variant('(a{sa{sv}}oo)', (settings, device_path, ap_path)),
                GLib.VariantType.new('(oo)'),
                Gio.DBusCallFlags.NONE,
                45000,
                None,
            ).unpack()
            wait_for_activation(bus, active_path, device_path=device_path)
        except GLib.Error:
            raise RuntimeError(generic_connect_error())

    def connect_with_secret(self, bssid, ssid, secret, key_mgmts):
        bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        device_path = nm_device_path(bus, self.iface)
        ap_path = find_ap_path(bus, device_path, bssid)
        for key_mgmt in key_mgmts:
            try:
                settings = build_psk_settings(ssid, secret, key_mgmt)
                _conn_path, active_path = bus.call_sync(
                    NM_DEST,
                    NM_ROOT,
                    NM_DEST,
                    'AddAndActivateConnection',
                    GLib.Variant('(a{sa{sv}}oo)', (settings, device_path, ap_path)),
                    GLib.VariantType.new('(oo)'),
                    Gio.DBusCallFlags.NONE,
                    45000,
                    None,
                ).unpack()
                wait_for_activation(bus, active_path, device_path=device_path)
                return
            except ConnectionFailure:
                raise
            except GLib.Error:
                continue
        raise ConnectionFailure(generic_connect_error())

    def handle_saved_profile_error(self, network, security, _exc):
        if security == 'psk':
            self.local_error = ''
            self.local_info = ''
            self.show_password_prompt(network, 'El perfil guardado falló. Ingresa la contraseña para reconectar.')
            return True
        self.local_error = generic_connect_error()
        return True

    def open_editor(self, *_):
        subprocess.Popen([sys.executable, str(ROOT / 'scripts/settings-launcher.py'), 'wifi'],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True,
            start_new_session=True)

    def network_button(self, net):
        button = Gtk.Button()
        if net.get('connected'):
            button.get_style_context().add_class('cc-network')
            button.get_style_context().add_class('selected')
        else:
            button.get_style_context().add_class('cc-network')
        button.set_sensitive(self.state.get('enabled', False) and not (self.state.get('busy', False) or self.local_busy) and not net.get('connected'))
        button.connect('clicked', self.on_network, net)

        row = Gtk.Box(spacing=10)
        icon = '󰤨' if net['signal'] > 65 else '󰤥' if net['signal'] > 35 else '󰤟'
        sig = Gtk.Label(label=icon)
        sig.get_style_context().add_class('cc-row-icon')
        name = Gtk.Label(label=net['name'])
        name.set_xalign(0)
        name.set_hexpand(True)
        name.set_ellipsize(3)
        pct = Gtk.Label(label=f"{net['signal']}%")
        pct.get_style_context().add_class('cc-secondary')
        lock = Gtk.Label(label='󰌾' if net.get('secure') else '󰌿')
        row.pack_start(sig, False, False, 0)
        row.pack_start(name, True, True, 0)
        row.pack_start(pct, False, False, 0)
        row.pack_start(lock, False, False, 0)
        button.add(row)
        return button

    def render(self):
        busy = bool(self.initializing or self.state.get('busy') or self.local_busy)
        self.toggle_btn.set_label('󰨚' if self.state.get('enabled') else '󰨙')
        ctx = self.toggle_btn.get_style_context()
        if self.state.get('enabled'):
            ctx.add_class('cc-toggle')
            ctx.add_class('on')
        else:
            ctx.add_class('cc-toggle')
            ctx.remove_class('on')
        self.toggle_btn.set_sensitive(self.state.get('available', False) and not busy)

        self.ssid_label.set_text(f"󰤨  {self.state.get('ssid', 'Sin conexión')}")
        self.status_label.set_text(f"{self.state.get('status', '')} · {self.state.get('signal', 0)}%")
        self.disconnect_btn.set_visible(self.state.get('signal', 0) > 0)
        self.disconnect_btn.set_sensitive(not busy)

        spinner = self.state.get('spinner', '󰑓')
        self.scan_btn.set_label('󰑓' if self.local_busy else spinner)
        if busy:
            self.scan_btn.get_style_context().add_class('cc-spinner')
        else:
            self.scan_btn.get_style_context().remove_class('cc-spinner')
        self.scan_btn.set_sensitive(self.state.get('enabled', False) and not busy)

        network_signature = tuple(
            (n.get('id'), n.get('name'), n.get('signal'), bool(n.get('secure')), bool(n.get('connected')), busy, bool(self.state.get('enabled', False)))
            for n in self.state.get('networks', [])
        )
        if network_signature != self.network_signature:
            for child in list(self.list_box.get_children()):
                self.list_box.remove(child)
            for net in self.state.get('networks', []):
                self.list_box.pack_start(self.network_button(net), False, False, 0)
            self.network_signature = network_signature
            self.list_box.show_all()

        if self.initializing:
            message = 'Cargando redes…'
            style = 'cc-info'
        elif self.local_busy and self.local_progress:
            message = self.local_progress
            style = 'cc-info'
        elif self.local_error:
            message = self.local_error
            style = 'cc-error'
        elif self.local_info:
            message = self.local_info
            style = 'cc-info'
        else:
            message = self.refresh_error or self.state.get('error', '')
            style = 'cc-error'
        self.error.set_text(message)
        self.error.set_visible(bool(message))
        self.loading_icon.set_visible(busy)
        self.feedback_box.set_visible(bool(message) or busy)
        css = self.error.get_style_context()
        css.remove_class('cc-error')
        css.remove_class('cc-info')
        css.add_class(style)


def parse_args(argv):
    if len(argv) != 6:
        raise ValueError('Uso: wifi-panel.py x y width maxheight screen monitor')
    x, y, width, maxheight, screen = [int(v) for v in argv[:5]]
    monitor = argv[5]
    return x, y, width, maxheight, screen, monitor


def write_pid_file():
    backend.RUN.mkdir(mode=0o700, exist_ok=True)
    tmp = PID_FILE.with_suffix(f'.{os.getpid()}.tmp')
    tmp.write_text(str(os.getpid()))
    tmp.replace(PID_FILE)


def clear_pid_file():
    PID_FILE.unlink(missing_ok=True)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    x, y, width, maxheight, screen, monitor = parse_args(argv)
    write_pid_file()
    signal.signal(signal.SIGTERM, lambda *_: Gtk.main_quit())
    signal.signal(signal.SIGINT, lambda *_: Gtk.main_quit())
    try:
        panel = WifiPanel(x, y, width, maxheight, screen, monitor)
        Gtk.main()
        panel.close()
    finally:
        clear_pid_file()


if __name__ == '__main__':
    try:
        main()
    except Exception as e:
        print(f'Wi-Fi panel: {e}', file=sys.stderr)
        sys.exit(1)

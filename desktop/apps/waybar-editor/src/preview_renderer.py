"""Isolated GTK3 design renderer. No Waybar process, commands, scripts or clicks.
Input is a JSON snapshot restricted to visual settings; stdout is a PNG.
"""
import datetime
import json
import os
import re
import sys
# This invisible image generator must not register transient accessibility apps.
# Accessibility belongs to the GTK4 picture and controls in the editor.
os.environ['NO_AT_BRIDGE'] = '1'
# Do not leave a CSS/resource reader behind if the editor closes or crashes.
parent = os.environ.get('WAYBAR_EDITOR_PREVIEW_PARENT')
if parent and sys.platform == 'linux':
    import ctypes
    import signal
    if ctypes.CDLL(None).prctl(1, signal.SIGTERM, 0, 0, 0) != 0:
        raise RuntimeError('No se pudo vincular la vista al editor')
    if os.getppid() != int(parent):
        sys.exit(0)
import gi

gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, Gdk, GLib, Pango


def sample_icon(spec, value=75, fallback=''):
    icons = spec.get('format-icons', fallback)
    if isinstance(icons, dict):
        icons = icons.get('default', fallback)
    if isinstance(icons, list):
        return str(icons[min(len(icons) - 1, int(value * len(icons) / 101))]) if icons else fallback
    return icons if isinstance(icons, str) else fallback


def sample_format(template, values):
    # Support common fmt placeholders without interpreting Python expressions.
    def replace(match):
        key, _, fmt = match.group(1).partition(':')
        if key == '' and fmt.startswith('%'):
            try:
                return datetime.datetime.now().strftime(fmt)
            except (ValueError, OverflowError):
                return '12:00'
        value = values.get(key, '…')
        if fmt and isinstance(value, (int, float)) and re.fullmatch(r'[0-9.]*[fd]?', fmt):
            try:
                return format(value, fmt)
            except (ValueError, OverflowError):
                pass
        return str(value)
    # Preserve literal escaped braces and Pango markup from the format.
    return re.sub(r'(?<!\{)\{([^{}]*)\}(?!\})', replace, str(template)).replace('{{', '{').replace('}}', '}')[:1024]


def label(text):
    widget = Gtk.Label()
    try:
        parsed = Pango.parse_markup(text, -1, '\x00')
        if parsed[0]:
            widget.set_markup(text)
        else:
            widget.set_text(text)
    except (GLib.Error, ValueError, AttributeError):
        widget.set_text(text)
    return widget


def module_widget(name, config, vertical, scenario, depth=0, budget=None):
    if budget is None:
        budget = [256]
    if budget[0] <= 0:
        return None
    budget[0] -= 1
    if depth > 8:
        return label('…')
    spec = config.get(name, {})
    if not isinstance(spec, dict):
        spec = {}
    base, _, instance = name.partition('#')
    identity = base.replace('/', '-') if base.startswith(('custom/', 'group/')) else base.split('/')[-1]
    orientation = Gtk.Orientation.VERTICAL if vertical else Gtk.Orientation.HORIZONTAL
    if base.endswith('/workspaces') or base == 'workspaces':
        widget = Gtk.Box(orientation=orientation)
        persistent = spec.get('persistent-workspaces', {})
        names = [str(k) for k in persistent if str(k).isdigit()] if isinstance(persistent, dict) else []
        names = sorted(names, key=int)[:24] or ['1', '2', '3', '4']
        for number in names:
            if budget[0] <= 0:
                break
            budget[0] -= 1
            icons = spec.get('format-icons', {})
            icon = icons.get(number, number) if isinstance(icons, dict) else number
            text = sample_format(spec.get('format', '{name}'), {'icon': icon, 'name': number, 'id': number, 'windows': ''})
            button = Gtk.Button()
            button.add(label(text))
            if number == names[min(2, len(names)-1)]:
                button.get_style_context().add_class('active')
                button.get_style_context().add_class('focused')
            widget.pack_start(button, False, False, 0)
    elif base.startswith('group/'):
        widget = Gtk.Box(orientation=orientation, spacing=int(spec.get('spacing', 0)))
        for child in spec.get('modules', [])[:40]:
            child_widget = module_widget(str(child), config, vertical, scenario, depth+1, budget)
            if child_widget is None:
                break
            widget.pack_start(child_widget, False, False, 0)
    elif base == 'tray':
        widget = Gtk.Box(orientation=orientation, spacing=max(0, min(64, int(spec.get('spacing', 8)))))
        image = Gtk.Image.new_from_icon_name('application-x-executable-symbolic', Gtk.IconSize.MENU)
        image.set_pixel_size(max(8, min(64, int(spec.get('icon-size', 16)))))
        widget.pack_start(image, False, False, 0)
    else:
        values = {'': '…', 'icon': '', 'capacity': 75, 'percent': 65, 'percentage': 35,
                  'volume': 46, 'usage': 23, 'temperatureC': 48, 'temperatureF': 118,
                  'essid': 'Mi Wi-Fi', 'signalStrength': 80, 'ipaddr': '192.168.1.10',
                  'used': 4.2, 'total': 16.0, 'avg': 1.2, 'frequency': 2.4,
                  'num_connections': 1, 'controller_alias': 'Bluetooth', 'title': 'Ventana de ejemplo'}
        defaults = {'battery': '{capacity}%', 'cpu': '{usage}%', 'memory': '{percentage}%',
                    'backlight': '{percent}%', 'clock': '{:%H:%M}', 'temperature': '{temperatureC}°C',
                    'pulseaudio': '{volume}%', 'network': '{ipaddr}', 'bluetooth': '󰂯',
                    'hyprland/window': '{title}', 'niri/window': '{title}', 'power-profiles-daemon': '{icon}'}
        template = spec.get('format', defaults.get(base, '{}'))
        classes = []
        if base == 'battery':
            capacity = {'warning': 25, 'critical': 10}.get(scenario, 75)
            values['capacity'] = capacity
            if scenario == 'charging':
                template = spec.get('format-charging', template)
                classes = ['charging']
            else:
                states = spec.get('states', {})
                if isinstance(states, dict):
                    eligible = [(v,k) for k,v in states.items() if isinstance(v, (int,float)) and capacity <= v]
                    classes = [min(eligible)[1]] if eligible else []
            values['icon'] = sample_icon(spec, capacity, '󰁿')
        elif base == 'network':
            template = spec.get('format-wifi', spec.get('format', '{essid}'))
            values['icon'] = sample_icon(spec, values['signalStrength'], '󰤨')
            classes = ['wifi', 'linked']
        elif base == 'pulseaudio':
            values['icon'] = sample_icon(spec, 46, '󰕾')
        elif base == 'backlight':
            values['icon'] = sample_icon(spec, 65, '󰃠')
        elif base == 'bluetooth':
            template = spec.get('format-connected', spec.get('format', '󰂯'))
            classes = ['on', 'connected']
        elif base == 'power-profiles-daemon':
            icons = spec.get('format-icons', {})
            values['icon'] = icons.get('balanced', '󰾅') if isinstance(icons, dict) else '󰾅'
            classes = ['balanced']
        elif base == 'custom/clock':
            now = datetime.datetime.now()
            days = ['lun','mar','mié','jue','vie','sáb','dom']
            months = ['ene','feb','mar','abr','may','jun','jul','ago','sep','oct','nov','dic']
            values[''] = f'{days[now.weekday()]} {now.day:02} {months[now.month-1]}  {now:%H:%M}'
        elif base == 'custom/airpods':
            values[''] = '󰋋 —'
            classes = ['disconnected']
        elif base == 'custom/media':
            # Empty example, like the module when no media player is active.
            values[''] = ''
        widget = label(sample_format(template, values))
        for c in classes:
            if re.fullmatch(r'[a-zA-Z0-9_-]+', c):
                widget.get_style_context().add_class(c)
        if not widget.get_text():
            widget.set_no_show_all(True)
            widget.hide()
    widget.set_name(identity)
    widget.get_style_context().add_class('module')
    if instance:
        widget.get_style_context().add_class(instance)
    return widget


def render(snapshot):
    config = snapshot['config']
    vertical = config.get('position', 'top') in ('left', 'right')
    orientation = Gtk.Orientation.VERTICAL if vertical else Gtk.Orientation.HORIZONTAL
    window = Gtk.OffscreenWindow()
    window.set_name('waybar')
    visual = window.get_screen().get_rgba_visual()
    if visual:
        window.set_visual(visual)
    window.get_style_context().add_class(config.get('position', 'top'))
    window.set_default_size(snapshot['width'], snapshot['height'])
    bar = Gtk.Box(orientation=orientation)
    bar.set_name('box')
    bar.get_style_context().add_class('horizontal' if not vertical else 'vertical')
    window.add(bar)
    zones = []
    budget = [256]
    for zone in ('left', 'center', 'right'):
        box = Gtk.Box(orientation=orientation, spacing=max(0,min(64,int(config.get('spacing', 0)))))
        box.get_style_context().add_class('modules-' + zone)
        for name in config.get('modules-' + zone, [])[:40]:
            widget = module_widget(str(name), config, vertical, snapshot.get('scenario','normal'), budget=budget)
            if widget is None:
                break
            box.pack_start(widget, False, False, 0)
        zones.append(box)
    bar.pack_start(zones[0], False, False, 0)
    if config.get('fixed-center', True):
        bar.set_center_widget(zones[1])
    else:
        bar.pack_start(zones[1], True, False, 0)
    bar.pack_end(zones[2], False, False, 0)
    provider = Gtk.CssProvider()
    provider.load_from_data(snapshot['css'].encode('utf-8'))
    Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_USER)
    window.show_all()
    # Give allocations and CSS transitions a deterministic initial frame.
    while Gtk.events_pending():
        Gtk.main_iteration()
    pixbuf = window.get_pixbuf()
    if pixbuf is None or pixbuf.get_width() > 8192 or pixbuf.get_height() > 4096:
        raise ValueError('Vista previa demasiado grande')
    ok, data = pixbuf.save_to_bufferv('png', [], [])
    if not ok:
        raise ValueError('No se pudo generar la imagen GTK3')
    window.destroy()
    Gtk.StyleContext.remove_provider_for_screen(Gdk.Screen.get_default(), provider)
    return data


if __name__ == '__main__':
    snapshot = json.load(sys.stdin)
    sys.stdout.buffer.write(render(snapshot))

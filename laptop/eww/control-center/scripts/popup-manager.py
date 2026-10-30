#!/usr/bin/env python3
"""Dedicated Eww config; single popup lock; actual pointer + layer geometry."""
import fcntl
import json
import os
from pathlib import Path
import signal
import shlex
import shutil
import subprocess
import sys
import time
import backend

WINDOWS = {m: 'cc-' + m for m in ('volume', 'wifi', 'brightness', 'calendar', 'bluetooth', 'airpods', 'profiles', 'nightlight', 'power', 'media')}
NATIVE_PANELS = {'wifi': backend.ROOT / 'scripts/wifi-panel.py',
                 'media': backend.ROOT.parents[1] / 'waybar/scripts/media-widget.py'}


def panel_pid_file(module):
    return backend.RUN / f'{module}-panel.pid'


def eww(*args, check=True):
    p = subprocess.run(['eww', '--force-wayland', '-c', str(backend.ROOT), *args], capture_output=True, text=True, timeout=10)
    if check and p.returncode:
        raise RuntimeError(p.stderr.strip() or p.stdout.strip())
    return p.stdout


def geometry(module, monitor=None, monitors=None):
    if monitors is None:
        monitors = json.loads(backend.command(['hyprctl', '-j', 'monitors']))
    cursor = json.loads(backend.command(['hyprctl', '-j', 'cursorpos']))
    selected = next((m for m in monitors if m['name'] == monitor), None) if monitor else None

    def size(m):
        w, h = m['width'], m['height']
        if m.get('transform', 0) % 2:
            w, h = h, w
        return w / m['scale'], h / m['scale']

    if not selected:
        selected = next((m for m in monitors if m['x'] <= cursor['x'] < m['x'] + size(m)[0] and m['y'] <= cursor['y'] < m['y'] + size(m)[1]), None)
    if not selected:
        selected = next((m for m in monitors if m.get('focused')), monitors[0])
    w, h = size(selected)
    settings = backend.config()
    width = min(settings['airpods_width'] if module == 'airpods' else settings['popup_width'], int(w) - 24)
    x = max(12, min(int(cursor['x'] - selected['x'] - width / 2), int(w) - width - 12))
    layers = json.loads(backend.command(['hyprctl', '-j', 'layers']))
    bars = [layer for level in layers.get(selected['name'], {}).get('levels', {}).values() for layer in level if layer.get('namespace') == 'waybar']
    bottom = max((layer['y'] - selected['y'] + layer['h'] for layer in bars if layer['y'] - selected['y'] < h / 2), default=46)
    y = int(bottom + 8)
    desired = {'volume': 330, 'wifi': 560, 'brightness': 520, 'calendar': 430, 'bluetooth': 480, 'airpods': 710, 'profiles': 500, 'nightlight': 350, 'power': 440, 'media': 260}[module]
    return selected['name'], x, y, width, min(desired, int(h - y - 12))


def screen_index(monitors, monitor):
    """Reuse GTK mapping only for an identical display session and topology."""
    signature = dict(
        display=os.environ.get('WAYLAND_DISPLAY'),
        session=os.environ.get('HYPRLAND_INSTANCE_SIGNATURE'),
        monitors=[{k: m.get(k) for k in ('id', 'name', 'description', 'x', 'y', 'width', 'height', 'scale', 'transform')} for m in monitors],
    )
    cache = backend.RUN / 'gtk-monitors.json'
    try:
        saved = json.loads(cache.read_text())
        if saved['signature'] == signature and time.time() - saved['time'] < 30:
            return saved['indexes'][monitor]
    except (OSError, ValueError, KeyError, TypeError):
        pass
    import gi

    gi.require_version('Gdk', '3.0')
    from gi.repository import Gdk

    Gdk.init([])
    display = Gdk.Display.get_default()
    if display is None:
        raise RuntimeError('No se pudo abrir la conexión GTK')
    indexes = {}
    for m in monitors:
        candidates = [
            i
            for i in range(display.get_n_monitors())
            if display.get_monitor(i).get_geometry().x == m['x'] and display.get_monitor(i).get_geometry().y == m['y']
        ]
        if len(candidates) != 1:
            raise RuntimeError('No se pudo resolver el monitor GTK por geometría')
        indexes[m['name']] = candidates[0]
    cache.write_text(json.dumps(dict(signature=signature, indexes=indexes, time=time.time())))
    return indexes[monitor]


def input_bindings(enable):
    # Keep our own Lua handles: never unbind the user's Escape/mouse shortcuts.
    code = ['for _, bind in ipairs(_G.waybar_control_center_binds or {}) do bind:unbind() end',
            '_G.waybar_control_center_binds = {}']
    if enable:
        state_path = backend.RUN / 'popup.json'
        state = json.loads(state_path.read_text())
        state['token'] = str(time.time_ns())
        state_path.write_text(json.dumps(state))
        script = shlex.quote(str(backend.ROOT / 'scripts/popup-manager.py'))
        command = json.dumps(f'{script} close {state["token"]}', ensure_ascii=False)
        # Read the press position in-process, before a slider drag can move it outside.
        code.append("""local function close_outside()
            local cursor = hl.get_cursor_pos()
            if not cursor then return end
            for _, panel in ipairs(hl.get_layers()) do
                if panel.mapped and panel.namespace == "waybar-control-center"
                    and panel.x <= cursor.x and cursor.x < panel.x + panel.w
                    and panel.y <= cursor.y and cursor.y < panel.y + panel.h then return end
            end
            hl.exec_cmd(""" + command + ") end")
        code.append(f'table.insert(_G.waybar_control_center_binds, hl.bind("Escape", hl.dsp.exec_cmd({command}), {{ transparent = true }}))')
        for button in range(272, 280):
            code.append(f'table.insert(_G.waybar_control_center_binds, hl.bind("mouse:{button}", close_outside, {{ non_consuming = true, ignore_mods = true, transparent = true }}))')
    backend.command(['hyprctl', 'eval', '; '.join(code)])
    (backend.RUN / 'input-bindings.json').unlink(missing_ok=True)


def _read_panel_pid(module):
    try:
        return int(panel_pid_file(module).read_text().strip())
    except (OSError, ValueError):
        return None


def _read_cmdline(pid):
    try:
        raw = Path(f'/proc/{pid}/cmdline').read_bytes()
    except OSError:
        return []
    return [arg for arg in raw.decode('utf-8', errors='ignore').split('\0') if arg]


def _is_panel_cmdline(cmdline, module):
    return str(NATIVE_PANELS[module]) in cmdline


def panel_running(module):
    pid = _read_panel_pid(module)
    if not pid:
        panel_pid_file(module).unlink(missing_ok=True)
        return False
    if _is_panel_cmdline(_read_cmdline(pid), module):
        return True
    panel_pid_file(module).unlink(missing_ok=True)
    return False


def close_native_panel(module, timeout=1.5):
    pid = _read_panel_pid(module)
    if not pid:
        panel_pid_file(module).unlink(missing_ok=True)
        return
    cmdline = _read_cmdline(pid)
    if not _is_panel_cmdline(cmdline, module):
        panel_pid_file(module).unlink(missing_ok=True)
        return
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        panel_pid_file(module).unlink(missing_ok=True)
        return
    deadline = time.time() + timeout
    while time.time() < deadline:
        if not Path(f'/proc/{pid}').exists():
            panel_pid_file(module).unlink(missing_ok=True)
            return
        time.sleep(0.05)
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    panel_pid_file(module).unlink(missing_ok=True)


def open_native_panel(module, x, y, width, maxheight, screen, monitor):
    close_native_panel(module)
    python = sys.executable if Path(sys.executable).name.startswith('python') else (shutil.which('python3') or '/usr/bin/python3')
    proc = subprocess.Popen(
        [python, str(NATIVE_PANELS[module]), str(x), str(y), str(width), str(maxheight), str(screen), monitor],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        close_fds=True,
    )
    deadline = time.time() + 2
    while time.time() < deadline:
        if proc.poll() is None and panel_running(module):
            return
        time.sleep(0.05)
    raise RuntimeError(f'No se pudo iniciar el popup {module}')


def outside(token=None):
    if not current_popup(token):
        return
    cursor = json.loads(backend.command(['hyprctl', '-j', 'cursorpos']))
    layers = json.loads(backend.command(['hyprctl', '-j', 'layers']))

    def contains(panel):
        return panel['x'] <= cursor['x'] < panel['x'] + panel['w'] and panel['y'] <= cursor['y'] < panel['y'] + panel['h']

    panels = [p for d in layers.values() for level in d['levels'].values() for p in level]
    if any(contains(p) for p in panels if p.get('namespace') == 'waybar-control-center'):
        return
    close()


def current_popup(token=None):
    try:
        state = json.loads((backend.RUN / 'popup.json').read_text())
        return token is None or state.get('token') == token
    except (OSError, ValueError):
        return False


def close(opened=None):
    input_bindings(False)
    for native in NATIVE_PANELS:
        close_native_panel(native)
    if opened is None:
        opened = eww('active-windows', check=False)
    ids = [line.split(':', 1)[0] for line in opened.splitlines() if line.split(':', 1)[0] in WINDOWS.values()]
    if ids:
        eww('close', *ids, check=False)
    (backend.RUN / 'scan-owned').unlink(missing_ok=True)
    (backend.RUN / 'popup.json').unlink(missing_ok=True)
    (backend.RUN / 'power-confirm.json').unlink(missing_ok=True)


def main(args=None):
    args = sys.argv[1:] if args is None else args
    mode = args[0] if args else 'close'
    with open(backend.RUN / 'popup.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if mode == 'cleanup':
            input_bindings(False)
            for native in NATIVE_PANELS:
                close_native_panel(native)
            (backend.RUN / 'popup.json').unlink(missing_ok=True)
            return
        if mode == 'outside':
            outside(args[1] if len(args) > 1 else None)
            return
        if mode == 'close':
            if len(args) == 1 or current_popup(args[1]):
                close()
            return
        module = {'nightlight': 'brightness'}.get(mode, mode)
        if module not in WINDOWS:
            raise ValueError('Popup inválido')

        if module in NATIVE_PANELS:
            opened = eww('active-windows', check=False)
            already = panel_running(module)
            close(opened)
            if already:
                return
            monitors = json.loads(backend.command(['hyprctl', '-j', 'monitors']))
            monitor, x, y, width, maxheight = geometry(module, args[1] if len(args) > 1 else None, monitors)
            (backend.RUN / 'monitor').write_text(monitor)
            (backend.RUN / 'popup.json').write_text(json.dumps(dict(module=module, monitor=monitor, x=x, y=y, width=width)))
            index = screen_index(monitors, monitor)
            hm = next(m for m in monitors if m['name'] == monitor)
            offset_y = max(0, y - hm.get('reserved', [0, 0, 0, 0])[1])
            open_native_panel(module, x, offset_y, width, maxheight, index, monitor)
            input_bindings(True)
            return

        try:
            opened = eww('active-windows')
        except RuntimeError:
            backend.command(['systemctl', '--user', 'start', 'waybar-control-center.service'])
            opened = eww('active-windows')
        already = any(l.split(':', 1)[0] == WINDOWS[module] for l in opened.splitlines())
        close(opened)
        if already:
            return
        monitors = json.loads(backend.command(['hyprctl', '-j', 'monitors']))
        monitor, x, y, width, maxheight = geometry(module, args[1] if len(args) > 1 else None, monitors)
        (backend.RUN / 'monitor').write_text(monitor)
        (backend.RUN / 'popup.json').write_text(json.dumps(dict(module=module, monitor=monitor, x=x, y=y, width=width)))
        hm = next(m for m in monitors if m['name'] == monitor)
        index = screen_index(monitors, monitor)
        offset_y = max(0, y - hm.get('reserved', [0, 0, 0, 0])[1])
        eww('open', WINDOWS[module], '--screen', str(index), '--pos', f'{x}x{offset_y}', '--size', f'{width}x1', '--arg', f'maxheight={maxheight}')
        input_bindings(True)


if __name__ == '__main__':
    try:
        main()
    except Exception as e:
        print(f'Control Center: {e}', file=sys.stderr)
        sys.exit(1)

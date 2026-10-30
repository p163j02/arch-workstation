#!/usr/bin/env python3
"""Dedicated Eww config; single popup lock; actual pointer + layer geometry."""
import fcntl
import json
import os
from pathlib import Path
import signal
import shutil
import shlex
import uuid
import subprocess
import sys
import time
import backend

WINDOWS = {m: 'cc-' + m for m in ('volume', 'wifi', 'brightness', 'calendar', 'bluetooth', 'airpods', 'profiles', 'nightlight', 'power')}
WIFI_PANEL = backend.ROOT / 'scripts' / 'wifi-panel.py'
NATIVE_PANELS = {'wifi': WIFI_PANEL, 'media': backend.ROOT.parents[1] / 'waybar' / 'scripts' / 'media-widget.py'}


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
    desired = {'volume': 330, 'wifi': 560, 'brightness': 520, 'calendar': 430, 'bluetooth': 480, 'airpods': 710, 'profiles': 500, 'nightlight': 350, 'power': 440, 'media': 300}[module]
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
    """Hyprland's Lua provider rejects legacy `keyword bind` commands."""
    command = ''
    if enable:
        state = json.loads((backend.RUN / 'popup.json').read_text())
        command = shlex.join([str(backend.ROOT / 'scripts' / 'popup-manager.py'), 'dismiss', state['generation']])
    script = backend.ROOT / 'scripts' / 'popup-input.lua'
    expression = f'local install = dofile({json.dumps(str(script))}); install({json.dumps(command)})'
    result = backend.command(['hyprctl', 'eval', expression])
    if result and result != 'ok':
        raise RuntimeError(result)
    (backend.RUN / 'input-bindings.json').unlink(missing_ok=True)


def _read_cmdline(pid):
    try:
        raw = Path(f'/proc/{pid}/cmdline').read_bytes()
    except OSError:
        return []
    return [arg for arg in raw.decode('utf-8', errors='ignore').split('\0') if arg]


def _is_native_panel_cmdline(module, cmdline):
    return str(NATIVE_PANELS[module]) in cmdline


def native_panel_pids(module):
    pid_file = backend.RUN / f'{module}-panel.pid'
    candidates = set()
    try:
        candidates.add(int(pid_file.read_text().strip()))
    except (OSError, ValueError):
        pass
    # Recover a mapped orphan left by an older manager/startup failure.
    try:
        layers = json.loads(backend.command(['hyprctl', '-j', 'layers']))
    except (RuntimeError, subprocess.TimeoutExpired):
        layers = {}
    for display in layers.values():
        for level in display['levels'].values():
            for panel in level:
                if panel.get('namespace') == 'waybar-control-center' and panel.get('pid', 0) > 0:
                    candidates.add(panel['pid'])
    return {pid for pid in candidates if _is_native_panel_cmdline(module, _read_cmdline(pid))}


def close_native_panel(module, timeout=1.5):
    pids = native_panel_pids(module)
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + timeout
    target = str(NATIVE_PANELS[module])
    while pids and time.monotonic() < deadline:
        pids = {pid for pid in pids if target in _read_cmdline(pid)}
        if pids:
            time.sleep(0.025)
    for pid in pids:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    (backend.RUN / f'{module}-panel.pid').unlink(missing_ok=True)
    (backend.RUN / f'{module}-panel.ready').unlink(missing_ok=True)


def open_native_panel(module, x, y, width, maxheight, screen, monitor, generation):
    python = sys.executable if Path(sys.executable).name.startswith('python') else (shutil.which('python3') or '/usr/bin/python3')
    ready = backend.RUN / f'{module}-panel.ready'
    pid_file = backend.RUN / f'{module}-panel.pid'
    ready.unlink(missing_ok=True)
    proc = subprocess.Popen(
        [python, str(NATIVE_PANELS[module]), str(x), str(y), str(width), str(maxheight), str(screen), monitor, generation],
        stdout=subprocess.DEVNULL,
        stderr=None,  # Startup failures must reach the dispatcher journal.
        close_fds=True,
    )
    try:
        # Register the child ourselves; never depend on a startup-time PID-file race.
        pid_file.write_text(str(proc.pid))
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                raise RuntimeError(f'El popup nativo {module} terminó durante el inicio')
            try:
                if ready.read_text().strip() == generation:
                    return
            except FileNotFoundError:
                pass
            time.sleep(0.025)
        raise RuntimeError(f'No se pudo iniciar el popup nativo {module}')
    except BaseException:
        # A slow/failed startup must never leave a window without dismissal binds.
        proc.terminate()
        try:
            proc.wait(timeout=1.5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        pid_file.unlink(missing_ok=True)
        ready.unlink(missing_ok=True)
        raise


def outside():
    if not (backend.RUN / 'popup.json').exists():
        return
    cursor = json.loads(backend.command(['hyprctl', '-j', 'cursorpos']))
    layers = json.loads(backend.command(['hyprctl', '-j', 'layers']))

    def contains(panel):
        return panel['x'] <= cursor['x'] < panel['x'] + panel['w'] and panel['y'] <= cursor['y'] < panel['y'] + panel['h']

    panels = [p for d in layers.values() for level in d['levels'].values() for p in level]
    if any(contains(p) for p in panels if p.get('namespace') == 'waybar-control-center'):
        return
    close()


def close(opened=None):
    # Remove the visible windows even if compositor input cleanup fails.
    try:
        input_bindings(False)
    finally:
        close_windows(opened)


def close_windows(opened=None):
    for module in NATIVE_PANELS:
        close_native_panel(module)
    if opened is None:
        opened = eww('active-windows', check=False)
    ids = [line.split(':', 1)[0] for line in opened.splitlines() if line.split(':', 1)[0] in WINDOWS.values()]
    if ids:
        eww('close', *ids)
    (backend.RUN / 'scan-owned').unlink(missing_ok=True)
    (backend.RUN / 'popup.json').unlink(missing_ok=True)
    (backend.RUN / 'power-confirm.json').unlink(missing_ok=True)


def main(args=None):
    args = sys.argv[1:] if args is None else args
    mode = args[0] if args else 'close'
    with open(backend.RUN / 'popup.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if mode == 'cleanup':
            close()
            return
        if mode == 'dismiss':
            try:
                state = json.loads((backend.RUN / 'popup.json').read_text())
            except FileNotFoundError:
                return
            # A delayed click/X/Escape must not close the replacement popup.
            if len(args) == 2 and args[1] == state.get('generation'):
                close()
            return
        if mode == 'outside':
            outside()
            return
        if mode == 'close':
            close()
            return
        module = {'nightlight': 'brightness'}.get(mode, mode)
        if module not in {*WINDOWS, *NATIVE_PANELS}:
            raise ValueError('Popup inválido')

        try:
            opened = eww('active-windows')
        except RuntimeError:
            backend.command(['systemctl', '--user', 'start', 'waybar-control-center.service'])
            opened = eww('active-windows')
        already = (bool(native_panel_pids(module)) if module in NATIVE_PANELS else
                   any(l.split(':', 1)[0] == WINDOWS[module] for l in opened.splitlines()))
        close(opened)
        if already:
            return
        monitors = json.loads(backend.command(['hyprctl', '-j', 'monitors']))
        monitor, x, y, width, maxheight = geometry(module, args[1] if len(args) > 1 else None, monitors)
        generation = uuid.uuid4().hex
        (backend.RUN / 'monitor').write_text(monitor)
        (backend.RUN / 'popup.json').write_text(json.dumps(dict(module=module, monitor=monitor, x=x, y=y, width=width, generation=generation)))
        try:
            # Arm dismissal before mapping the new window.
            input_bindings(True)
            hm = next(m for m in monitors if m['name'] == monitor)
            index = screen_index(monitors, monitor)
            offset_y = max(0, y - hm.get('reserved', [0, 0, 0, 0])[1])
            if module in NATIVE_PANELS:
                open_native_panel(module, x, offset_y, width, maxheight, index, monitor, generation)
            else:
                eww('open', WINDOWS[module], '--screen', str(index), '--pos', f'{x}x{offset_y}', '--size', f'{width}x1', '--arg', f'maxheight={maxheight}', '--arg', f'generation={generation}')
        except BaseException:
            close()
            raise


if __name__ == '__main__':
    try:
        main()
    except Exception as e:
        print(f'Control Center: {e}', file=sys.stderr)
        sys.exit(1)

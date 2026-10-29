#!/usr/bin/env python3
"""RAM occupancy and real root-disk throughput streams for Eww."""
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from monitor_theme import chart_colors


def root_devices(sysfs=Path('/sys/dev/block'), root=Path('/')):
    """Resolve the root partition to its disk; unwrap stacked devices once."""
    dev = root.stat().st_dev
    target = sysfs / f'{os.major(dev)}:{os.minor(dev)}'
    if not target.exists():
        # Some filesystems expose an anonymous st_dev (e.g. Btrfs).
        source = subprocess.check_output(['findmnt', '-n', '-o', 'SOURCE', '/'], text=True, timeout=3).strip().split('[')[0]
        dev = Path(source).stat().st_rdev
        target = sysfs / f'{os.major(dev)}:{os.minor(dev)}'
    if not target.exists():
        raise OSError('no se pudo identificar el dispositivo de /')

    def leaves(path, seen):
        path = path.resolve()
        if path in seen:
            return set()
        seen.add(path)
        if (path / 'partition').exists():
            path = path.parent
        slaves = list((path / 'slaves').iterdir()) if (path / 'slaves').exists() else []
        return set().union(*(leaves(p, seen) for p in slaves)) if slaves else {path}
    return sorted(leaves(target, set()))


def disk_counters(devices):
    """Linux block stat sectors are always 512 bytes, even on 4K drives."""
    read_bytes = write_bytes = 0
    for device in devices:
        fields = [int(v) for v in (device / 'stat').read_text().split()]
        read_bytes += fields[2] * 512
        write_bytes += fields[6] * 512
    return read_bytes, write_bytes


def throughput(previous, current, elapsed):
    """None means no valid interval, not an invented zero measurement."""
    if previous is None or elapsed <= 0 or any(b < a for a, b in zip(previous, current)):
        return None
    return tuple((b - a) / elapsed for a, b in zip(previous, current))


def format_rate(value):
    if value is None:
        return '—'
    for divisor, unit in ((1024**3, 'GiB/s'), (1024**2, 'MiB/s'), (1024, 'KiB/s')):
        if value >= divisor:
            return f'{value / divisor:.1f} {unit}'
    return f'{value:.0f} B/s'


def disk_chart(values):
    """Total read + write bytes/s, autoscaled; never plots capacity percent."""
    width, height = 304, 48
    colors = chart_colors()
    peak = max(values, default=0)
    ceiling = max(1024**2, 2 ** math.ceil(math.log2(max(1, peak))))
    points = [(i * width / max(1, len(values) - 1), height - 2 - v / ceiling * (height - 8)) for i, v in enumerate(values)]
    if len(points) == 1:
        drawing = f'<circle cx="1" cy="{points[0][1]:.1f}" r="1.5" fill="{colors["primary"]}"/>'
    elif points:
        line = ' '.join(f'{x:.1f},{y:.1f}' for x, y in points)
        drawing = (f'<polygon points="0,{height} {line} {width},{height}" fill="url(#fill)"/>'
                   f'<polyline points="{line}" fill="none" stroke="{colors["primary"]}" stroke-width="2" stroke-linejoin="round"/>')
    else:
        drawing = ''
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">'
           f'<defs><linearGradient id="fill" x1="0" y1="0" x2="0" y2="1"><stop stop-color="{colors["primary"]}" stop-opacity=".5"/>'
           f'<stop offset="1" stop-color="{colors["primary"]}" stop-opacity=".04"/></linearGradient></defs>{drawing}</svg>')
    return svg, ceiling


def occupancy(kind):
    source = Path(__file__).with_name(f'{kind}-info.sh')
    data = json.loads(subprocess.check_output(['bash', str(source)], timeout=5))
    key = 'ram_pct' if kind == 'ram' else 'pct'
    pct = data[key] = max(0, min(100, int(data[key])))
    normal = 60 if kind == 'ram' else 65
    data.update(status='Normal' if pct <= normal else 'Elevada' if pct <= 85 else 'Alta',
                level='normal' if pct <= normal else 'elevated' if pct <= 85 else 'high')
    return {key: value for key, value in data.items() if not key.startswith('swap_')}


def main(kind):
    if kind == 'ram':
        while True:
            try:
                print(json.dumps(occupancy(kind)), flush=True)
            except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
                print(f'ram-activity: {error}', file=sys.stderr)
            time.sleep(3)
    else:
        values, previous, previous_time = [], None, None
        capacity = dict(used_gib='—', total_gib='—', pct=0, fstype='—', status='No disponible', level='normal')
        refreshed = -float('inf')
        with tempfile.TemporaryDirectory(prefix='eww-disk-', dir=os.environ.get('XDG_RUNTIME_DIR')) as runtime:
            frame = 0
            devices = None
            while True:
                now = time.monotonic()
                try:
                    if now - refreshed >= 30 or not capacity:
                        capacity = occupancy('disk')
                        refreshed = now
                    if devices is None:
                        devices = root_devices()
                    current = disk_counters(devices)
                    sampled = time.monotonic()
                    rates = throughput(previous, current, sampled - previous_time if previous_time is not None else 0)
                    previous, previous_time = current, sampled
                    data = dict(capacity, device=', '.join(p.name for p in devices), graph='',
                                read_rate='—', write_rate='—', ceiling='—', io_status='Esperando primera muestra')
                    if rates is not None:
                        values = (values + [sum(rates)])[-60:]
                        svg, ceiling = disk_chart(values)
                        path = Path(runtime) / f'io-{frame % 2}.svg'
                        path.write_text(svg)
                        data.update(graph=str(path), read_bps=rates[0], write_bps=rates[1],
                                    read_rate=format_rate(rates[0]), write_rate=format_rate(rates[1]),
                                    ceiling=format_rate(ceiling), io_status='Lectura + escritura reales')
                        frame += 1
                    else:
                        values = []
                    print(json.dumps(data), flush=True)
                except (OSError, ValueError, KeyError, IndexError, subprocess.SubprocessError) as error:
                    print(f'disk-activity: {error}', file=sys.stderr)
                    devices, previous, previous_time, values = None, None, None, []
                    print(json.dumps(dict(capacity, graph='', device='—', read_rate='—', write_rate='—',
                                          ceiling='—', io_status='E/S no disponible')), flush=True)
                time.sleep(2)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    if len(sys.argv) != 2 or sys.argv[1] not in ('ram', 'disk'):
        raise SystemExit('Usage: resource-activity.py ram|disk')
    try:
        main(sys.argv[1])
    except (BrokenPipeError, KeyboardInterrupt):
        pass

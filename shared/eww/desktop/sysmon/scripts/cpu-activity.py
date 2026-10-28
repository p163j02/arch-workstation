#!/usr/bin/env python3
"""Stream aggregate CPU usage, physical cores, mean frequency and real history."""
import json
import math
import os
import signal
from pathlib import Path
import sys
import tempfile
import time
from monitor_theme import chart_colors


def counters(path=Path('/proc/stat')):
    # guest/guest_nice are already included in user/nice: do not count twice.
    fields = [int(v) for v in path.read_text().splitlines()[0].split()[1:9]]
    return sum(fields), fields[3] + fields[4]


def utilization(previous, current):
    total = current[0] - previous[0]
    idle = current[1] - previous[1]
    return round(max(0, min(100, 100 * (total - idle) / total))) if total > 0 else 0


def hardware():
    info = Path('/proc/cpuinfo').read_text()
    cores, frequencies = set(), []
    for block in info.strip().split('\n\n'):
        fields = dict(line.split(':', 1) for line in block.splitlines() if ':' in line)
        fields = {key.strip(): value.strip() for key, value in fields.items()}
        if 'core id' in fields:
            cores.add((fields.get('physical id', '0'), fields['core id']))
        if 'cpu MHz' in fields:
            frequencies.append(float(fields['cpu MHz']))
    return {
        'cores': len(cores),
        'threads': os.cpu_count() or 1,
        'ghz': f'{sum(frequencies) / len(frequencies) / 1000:.1f}' if frequencies else '—',
    }


def chart(values):
    width, height = 304, 52
    colors = chart_colors()
    ceiling = max(10, math.ceil(max(values, default=0) / 10) * 10)
    denominator = max(1, len(values) - 1)
    points = [(i * width / denominator, height - 2 - value / ceiling * (height - 8))
              for i, value in enumerate(values)]
    if len(points) == 1:
        # Show the first real sample as a point, not an invented history.
        x, y = points[0]
        drawing = f'<circle cx="{x + 1}" cy="{y:.1f}" r="1.5" fill="#35e7ff"/>'
    else:
        line = ' '.join(f'{x:.1f},{y:.1f}' for x, y in points)
        drawing = (f'<polygon points="0,{height} {line} {width},{height}" fill="url(#fill)"/>'
                   f'<polyline points="{line}" fill="none" stroke="#35e7ff" stroke-width="1.6" stroke-linejoin="round"/>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">'
            '<defs><linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">'
            '<stop stop-color="#19baff" stop-opacity=".45"/>'
            '<stop offset="1" stop-color="#137bc2" stop-opacity=".08"/></linearGradient></defs>'
            f'{drawing}</svg>').replace('#35e7ff', colors['primary']).replace('#19baff', colors['primary']).replace('#137bc2', colors['primary']), ceiling


def main():
    values = []
    previous = counters()
    with tempfile.TemporaryDirectory(prefix='eww-cpu-', dir=os.environ.get('XDG_RUNTIME_DIR')) as runtime:
        frame = 0
        while True:
            time.sleep(2)
            try:
                current = counters()
                pct = utilization(previous, current)
                previous = current
                values = (values + [pct])[-60:]
                svg, ceiling = chart(values)
                path = Path(runtime) / f'activity-{frame % 2}.svg'
                path.write_text(svg)
                data = hardware()
                data.update(pct=pct, graph=str(path), ceiling=ceiling,
                            status='Normal' if pct <= 50 else 'Elevada' if pct <= 80 else 'Alta',
                            level='normal' if pct <= 50 else 'elevated' if pct <= 80 else 'high')
                print(json.dumps(data), flush=True)
                frame += 1
            except (OSError, ValueError, IndexError) as error:
                print(f'cpu-activity: {error}', file=sys.stderr)


if __name__ == '__main__':
    # Eww stops listeners with SIGTERM; unwind the private runtime directory.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    try:
        main()
    except (BrokenPipeError, KeyboardInterrupt):
        pass

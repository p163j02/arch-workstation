#!/usr/bin/env python3
"""Stream real temperature readings and bounded SVG histories to Eww."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from monitor_theme import chart_colors


def chart(values, color):
    points = [(i * 112 / max(1, len(values) - 1), 23 - max(0, min(1, (v - 20) / 80)) * 21)
              for i, v in enumerate(values)]
    line = ' '.join(f'{x:.1f},{y:.1f}' for x, y in points)
    end = points[-1][0]
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="112" height="24" viewBox="0 0 112 24">'
            f'<defs><linearGradient id="fill" x2="0" y2="1"><stop stop-color="{color}" stop-opacity=".22"/>'
            f'<stop offset="1" stop-color="{color}" stop-opacity="0"/></linearGradient></defs>'
            f'<polygon points="0,24 {line} {end:.1f},24" fill="url(#fill)"/>'
            f'<polyline points="{line}" fill="none" stroke="{color}" stroke-width="1.2"/></svg>')


def main():
    history = {'cpu': [], 'gpu': []}
    script = Path(__file__).with_name('temps.sh')
    # Private runtime directory; no persistent writes and no shared filenames.
    with tempfile.TemporaryDirectory(prefix='eww-temp-', dir=os.environ.get('XDG_RUNTIME_DIR')) as runtime:
        frame = 0
        while True:
            try:
                data = json.loads(subprocess.check_output(['bash', str(script)], timeout=4))
                for sensor in history:
                    value = data[sensor]
                    tokens = chart_colors()
                    color = tokens['danger'] if value > 85 else tokens['warning'] if value > 70 else tokens['soft']
                    data[f'{sensor}_color'] = color
                    history[sensor] = (history[sensor] + [value])[-24:]
                    path = Path(runtime) / f'{sensor}-{frame % 2}.svg'
                    path.write_text(chart(history[sensor], color))
                    data[f'{sensor}_graph'] = str(path)
                print(json.dumps(data), flush=True)
                frame += 1
            except (subprocess.SubprocessError, ValueError, KeyError, OSError) as error:
                import sys
                print(f'temp-history: {error}', file=sys.stderr)
            time.sleep(5)


if __name__ == '__main__':
    try:
        main()
    except (BrokenPipeError, KeyboardInterrupt):
        pass

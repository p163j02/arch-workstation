#!/usr/bin/env python3
"""Read available CPU/GPU sensors; -1 means unavailable, never a fabricated 0 C."""
import json
from pathlib import Path
import shutil
import subprocess


def temperatures(hwmon=Path('/sys/class/hwmon')):
    result = {'cpu': -1, 'gpu': -1}
    for device in hwmon.glob('hwmon*'):
        try:
            name = (device / 'name').read_text().strip()
            value = int((device / 'temp1_input').read_text()) // 1000
        except (OSError, ValueError):
            continue
        if name in ('k10temp', 'coretemp'):
            result['cpu'] = max(result['cpu'], value)
        elif name == 'amdgpu':
            result['gpu'] = max(result['gpu'], value)
    if shutil.which('nvidia-smi'):
        try:
            output = subprocess.check_output(['nvidia-smi', '--query-gpu=temperature.gpu',
                '--format=csv,noheader,nounits'], text=True, stderr=subprocess.DEVNULL, timeout=3)
            values = [int(line.strip()) for line in output.splitlines() if line.strip().isdigit()]
            if values:
                result['gpu'] = max(values)
        except (OSError, subprocess.SubprocessError):
            pass
    return result


if __name__ == '__main__':
    print(json.dumps(temperatures()))

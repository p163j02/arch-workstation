#!/usr/bin/env python3
"""Apply the existing monitor specification through Hyprland's native Lua API."""
import json
from pathlib import Path
import subprocess
import sys


def apply(specification):
    reader = json.dumps(str(Path(__file__).resolve().with_name('local-settings.lua')), ensure_ascii=False)
    value = json.dumps(specification, ensure_ascii=False)
    result = subprocess.run(['hyprctl', 'eval', f'dofile({reader}).monitor({value})'], capture_output=True, text=True)
    if result.returncode or result.stdout.strip() != 'ok':
        raise RuntimeError(result.stderr or result.stdout or 'Hyprland did not apply the monitor')


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2:
            raise ValueError('Uso: monitor-set.py OUTPUT,MODE,POSITION,SCALE[,mirror,OUTPUT]')
        apply(sys.argv[1])
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)

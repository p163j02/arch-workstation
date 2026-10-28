#!/usr/bin/env python3
"""Stream Waybar Editor's effective contour and derived palette without reloading Eww."""
import json
from pathlib import Path
import re
import signal
import sys
import time
from monitor_theme import palette

NUMBER = r'-?\d+(?:\.\d+)?'
COLOR = r'(?:\#[0-9a-fA-F]{6}|\#[0-9a-fA-F]{3}|rgba?\([\d.,\s]+\))'
FALLBACK = ('border: 1px solid rgba(64, 191, 191, 0.500); border-radius: 20px; '
            'box-shadow: 0 0 6px rgba(64, 191, 191, 0.300), inset 0 0 4px rgba(64, 191, 191, 0.300);')


def valid_color(value):
    if not re.fullmatch(COLOR, value):
        raise ValueError('Unsupported border color')
    if value.startswith('#'):
        return value
    channels = [float(x.strip()) for x in value[value.index('(') + 1:-1].split(',')]
    count = 4 if value.startswith('rgba(') else 3
    if len(channels) != count or not all(0 <= n <= 255 for n in channels[:3]):
        raise ValueError('Invalid RGB channels')
    if count == 4 and not 0 <= channels[3] <= 1:
        raise ValueError('Invalid alpha')
    return value


def valid_shadow(value):
    if value == 'none':
        return value
    shadows = re.split(r',\s*(?![^()]*\))', value)
    if not 1 <= len(shadows) <= 2:
        raise ValueError('Invalid shadow count')
    for shadow in shadows:
        match = re.fullmatch(rf'((?:inset\s+)?(?:{NUMBER}(?:px)?\s+){{3,4}})({COLOR})', shadow.strip())
        if not match:
            raise ValueError('Invalid shadow')
        lengths = match[1].replace('inset', '').split()
        values = [float(n.removesuffix('px')) for n in lengths]
        if not all(-100 <= n <= 100 for n in values) or values[2] < 0:
            raise ValueError('Invalid shadow geometry')
        valid_color(match[2])
    return value


def contour(text):
    """Use rendered CSS, not stale original rules or a guessed glow formula."""
    managed = re.search(r'/\* waybar-editor:begin \*/(.*?)/\* waybar-editor:end \*/', text, re.S)
    if not managed:
        raise ValueError('Incomplete Waybar Editor block')
    metadata = re.search(r'/\* waybar-editor:appearance (.*?) \*/', managed[1], re.S)
    if not metadata or not isinstance(json.loads(metadata[1]), dict):
        raise ValueError('Incomplete appearance metadata')
    block = re.search(r'window#waybar\s+\.modules-left\s*,([^{}]*)\{([^{}]*)\}', managed[1], re.S)
    if not block or '.modules-center' not in block[1] or '.modules-right' not in block[1]:
        raise ValueError('Missing shared module contour')
    declarations = {}
    for declaration in block[2].split(';'):
        if ':' in declaration:
            key, value = declaration.split(':', 1)
            declarations[key.strip()] = value.strip()
    border = declarations['border']
    match = re.fullmatch(rf'({NUMBER})px\s+solid\s+({COLOR})', border)
    if not match or not 0 <= float(match[1]) <= 20:
        raise ValueError('Invalid border width')
    valid_color(match[2])
    radius = declarations['border-radius']
    match = re.fullmatch(rf'({NUMBER})px', radius)
    if not match or not 0 <= float(match[1]) <= 100:
        raise ValueError('Invalid border radius')
    shadow = valid_shadow(declarations['box-shadow'])
    return f'border: {border}; border-radius: {radius}; box-shadow: {shadow};'


def styles(path, interval=0.5):
    """Only stat an unchanged file; detect atomic replacements and retain last good style."""
    previous_signature = object()
    previous_style = None
    while True:
        try:
            stat = path.stat()
            signature = (stat.st_ino, stat.st_mtime_ns, stat.st_size)
        except OSError:
            signature = None
        if signature != previous_signature:
            previous_signature = signature
            try:
                style = contour(path.read_text())
            except (OSError, UnicodeError, ValueError, KeyError):
                style = previous_style or FALLBACK
            if style != previous_style:
                previous_style = style
                yield style
        time.sleep(interval)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / '.config/waybar/style.css'
    try:
        for style in styles(path):
            print(json.dumps(palette(style)), flush=True)
    except (BrokenPipeError, KeyboardInterrupt):
        pass

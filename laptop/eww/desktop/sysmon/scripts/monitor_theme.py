"""Shared visual tokens derived from the validated, effective Waybar contour."""
import importlib.util
import json
import os
from pathlib import Path
import re


def wallpaper_colors(text):
    """Read only a complete managed block and validate every exported color."""
    start, end = '/* wallpaper-studio:begin */', '/* wallpaper-studio:end */'
    if start not in text and end not in text:
        return None
    block = re.search(re.escape(start) + r'(.*?)' + re.escape(end), text, re.S)
    if not block:
        raise ValueError('Incomplete Wallpaper Studio block')
    match = re.search(r'/\* wallpaper-studio:colors (\{.*?\}) \*/', block[1])
    if not match:
        raise ValueError('Missing Wallpaper Studio palette')
    colors = json.loads(match[1])
    if not isinstance(colors, dict) or not all(
        isinstance(colors.get(key), str) and re.fullmatch(r'#[0-9a-fA-F]{6}', colors[key])
        for key in ('accent', 'secondary', 'background', 'foreground')
    ):
        raise ValueError('Invalid Wallpaper Studio palette')
    return colors


def palette(style, colors=None):
    color = re.search(r'border: [\d.]+px solid (.*?);', style)[1]
    if color.startswith('#'):
        value = color[1:]
        if len(value) == 3:
            value = ''.join(c * 2 for c in value)
        rgb = tuple(int(value[i:i + 2], 16) for i in (0, 2, 4))
    else:
        rgb = tuple(round(float(n)) for n in color[color.index('(') + 1:-1].split(',')[:3])

    def mix(target, amount):
        return '#' + ''.join(f'{round(a * (1 - amount) + b * amount):02x}' for a, b in zip(rgb, target))

    # Preserve Waybar's exact RGB accent; alpha is for the contour, not text.
    primary = mix((255, 255, 255), 0)
    tokens = dict(style=style, primary=primary, soft=mix((255, 255, 255), .55),
                text=mix((255, 255, 255), .92), muted=mix((200, 215, 230), .72),
                background=mix((5, 10, 18), .96), card=mix((15, 23, 34), .89),
                track=mix((30, 40, 53), .86), separator=mix((32, 44, 56), .68),
                warning='#ebcb8b', danger='#ff7189')
    if colors:
        tokens.update(primary=colors['accent'], soft=colors['secondary'],
                      text=colors['foreground'], background=colors['background'])
        def blend(a, b, amount):
            a = tuple(int(a[i:i + 2], 16) for i in (1, 3, 5))
            b = tuple(int(b[i:i + 2], 16) for i in (1, 3, 5))
            return '#' + ''.join(f'{round(x * (1 - amount) + y * amount):02x}' for x, y in zip(a, b))
        bg, fg = colors['background'], colors['foreground']
        tokens.update(card=blend(bg, fg, .06), track=blend(bg, fg, .12),
                      separator=blend(bg, colors['accent'], .30), muted=blend(bg, fg, .72))
    return tokens


class Theme:
    """Stat-only on unchanged CSS; retain last valid palette on incomplete writes."""
    def __init__(self, path=None):
        self.path = path or Path(os.environ.get('XDG_CONFIG_HOME') or Path.home() / '.config') / 'waybar/style.css'
        spec = importlib.util.spec_from_file_location('waybar_border_parser', Path(__file__).with_name('waybar-border.py'))
        self.parser = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.parser)
        self.tokens = palette(self.parser.FALLBACK)
        self.signature = object()

    def read(self):
        try:
            stat = self.path.stat()
            signature = (stat.st_ino, stat.st_mtime_ns, stat.st_size)
        except OSError:
            signature = None
        if signature != self.signature:
            self.signature = signature
            try:
                text = self.path.read_text()
                self.tokens = self.parser.tokens(text)
            except (OSError, UnicodeError, ValueError, KeyError, TypeError):
                pass
        return self.tokens


_theme = None


def chart_colors():
    global _theme
    if _theme is None:
        _theme = Theme()
    return _theme.read()

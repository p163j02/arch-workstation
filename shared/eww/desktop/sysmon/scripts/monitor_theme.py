"""Shared visual tokens derived from the validated, effective Waybar contour."""
import importlib.util
from pathlib import Path
import re


def palette(style):
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
    return dict(style=style, primary=primary, soft=mix((255, 255, 255), .55),
                text=mix((255, 255, 255), .92), muted=mix((200, 215, 230), .72),
                background=mix((5, 10, 18), .96), card=mix((15, 23, 34), .89),
                track=mix((30, 40, 53), .86), separator=mix((32, 44, 56), .68),
                warning='#ebcb8b', danger='#ff7189')


class Theme:
    """Stat-only on unchanged CSS; retain last valid palette on incomplete writes."""
    def __init__(self, path=None):
        self.path = path or Path.home() / '.config/waybar/style.css'
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
                self.tokens = palette(self.parser.contour(self.path.read_text()))
            except (OSError, UnicodeError, ValueError, KeyError):
                pass
        return self.tokens


_theme = None


def chart_colors():
    global _theme
    if _theme is None:
        _theme = Theme()
    return _theme.read()

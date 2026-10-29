#!/usr/bin/env python3
"""Render portable home placeholders, or normalize a local snapshot before saving it."""
from pathlib import Path
import json
import sys


def transform(root, home, render=False):
    for path in root.rglob('*'):
        if path.is_symlink() or not path.is_file():
            continue
        try:
            text = path.read_text()
        except UnicodeError:
            continue
        if '\0' in text:
            continue
        if not render and text.startswith('#!') and text.splitlines()[0].endswith('/fast-client'):
            text = '#!/usr/bin/env python3\n' + text.split('\n', 1)[1]
        if not render and path.name == 'settings.json' and path.parent.name == 'control-center':
            settings = json.loads(text)
            for key in ('backlight_device', 'backlight_monitor', 'airpods_address', 'external_connector', 'ddc_bus'):
                if key in settings:
                    settings[key] = ''
            text = json.dumps(settings, ensure_ascii=False, indent=2) + '\n'
        before, after = ('@HOME@', home) if render else (home, '@HOME@')
        updated = text.replace(before, after)
        if updated != path.read_text():
            path.write_text(updated)


if __name__ == '__main__':
    mode, directory, home = sys.argv[1:]
    if mode not in ('render', 'normalize'):
        raise SystemExit('Expected render or normalize')
    transform(Path(directory), home, render=mode == 'render')

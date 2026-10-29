#!/usr/bin/env python3
"""Capture reproducible notebook settings into a staged laptop tree."""
import configparser
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

from check import SECRET_PATTERNS, SENSITIVE_KEYS, jsonc

PREFERENCES = (
    'htop/htoprc', 'lazydocker/config.yml', 'airpods-tui/config.toml',
    'Thunar/uca.xml', 'Thunar/accels.scm', 'xfce4/xfconf/xfce-perchannel-xml/thunar.xml',
    'pavucontrol.ini', 'QtProject.conf', 'user-dirs.dirs',
    'Code/User/settings.json', 'Code/User/keybindings.json', 'Code/User/snippets',
    'Antigravity/User/settings.json', 'Antigravity/User/keybindings.json', 'Antigravity/User/snippets',
    'Antigravity IDE/User/settings.json', 'Antigravity IDE/User/keybindings.json',
    'Antigravity IDE/User/snippets', 'obs-studio/global.ini', 'obs-studio/user.ini',
)
# The installed binary package was removed from AUR; retain its version in inventory.
AUR_REPLACEMENTS = {'claude-desktop-bin': 'claude-desktop'}


def copy_file(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def has_credentials(value):
    if isinstance(value, dict):
        return any((SENSITIVE_KEYS.fullmatch(key) and isinstance(item, str) and item
                    and not re.fullmatch(r'\{env:[A-Z_][A-Z0-9_]*\}', item))
                   or has_credentials(item) for key, item in value.items())
    return isinstance(value, list) and any(has_credentials(item) for item in value)


def capture(root, home):
    config = Path(os.environ.get('XDG_CONFIG_HOME') or home / '.config')
    inventory = root / 'inventory'
    inventory.mkdir(exist_ok=True)
    omitted = []
    for name in PREFERENCES:
        source = config / name
        if source.is_dir():
            for file in source.rglob('*.json'):
                copy_file(file, root / 'preferences' / file.relative_to(config))
        elif source.is_file():
            copy_file(source, root / 'preferences' / name)
    if (config / 'mimeapps.list').is_file():
        copy_file(config / 'mimeapps.list', root / 'mimeapps.list')

    explicit = set(subprocess.check_output(['pacman', '-Qqen'], text=True).split())
    installed = set(subprocess.check_output(['pacman', '-Qqn'], text=True).split())
    previous = set((root / 'packages/official.txt').read_text().split())
    required = {'base-devel', 'git', 'python', 'rsync', 'fish', 'lua', 'pkgconf', 'rust', 'fuse2',
                'gtk3', 'gtk4', 'libadwaita', 'python-gobject', 'python-cairo', 'gtk-layer-shell',
                'libnm', 'hyprpolkitagent', 'hyprsunset', 'playerctl'}
    official = explicit | (previous & installed) | required
    (root / 'packages/official.txt').write_text('\n'.join(sorted(official)) + '\n')
    foreign = subprocess.check_output(['pacman', '-Qqem'], text=True).split()
    aur = {AUR_REPLACEMENTS.get(name, name) for name in foreign if name != 'yay'}
    (root / 'packages/aur.txt').write_text('\n'.join(sorted(aur)) + '\n')
    (inventory / 'installed-versions.txt').write_text(subprocess.check_output(['pacman', '-Q'], text=True))
    if shutil.which('flatpak'):
        result = subprocess.run(['flatpak', 'list', '--app', '--columns=application,origin'],
                                capture_output=True, text=True, check=True)
        (root / 'packages/flatpak.tsv').write_text(result.stdout)

    enabled = set()
    for file in Path('/etc/systemd/system').rglob('*.service'):
        if file.is_symlink() and '.wants' in str(file.parent):
            name = file.resolve().name
            if '@' not in name:
                enabled.add(name)
    enabled.add('sddm.service')
    (root / 'system/enabled-services.txt').write_text('\n'.join(sorted(enabled)) + '\n')
    for source, target in [('/etc/systemd/zram-generator.conf', 'system/zram-generator.conf'),
                           ('/etc/sysctl.d/99-swappiness.conf', 'system/99-swappiness.conf'),
                           ('/etc/sddm.conf.d/theme.conf', 'sddm/theme.conf')]:
        if Path(source).is_file():
            copy_file(Path(source), root / target)
    theme = Path('/usr/share/sddm/themes/pixie')
    if theme.is_dir():
        subprocess.run(['rsync', '-a', '--no-owner', '--no-group', '--exclude=theme.conf.user',
                        '--exclude=*.bak*', str(theme) + '/', str(root / 'sddm/pixie') + '/'], check=True)
    appearance = configparser.ConfigParser(interpolation=None)
    appearance['org.gnome.desktop.interface'] = {}
    with tempfile.TemporaryDirectory(prefix='notebook-gsettings-') as runtime:
        for key in ('gtk-theme', 'icon-theme', 'cursor-theme', 'cursor-size', 'font-name', 'color-scheme'):
            value = subprocess.check_output(['gsettings', 'get', 'org.gnome.desktop.interface', key],
                                            text=True, stderr=subprocess.PIPE,
                                            env=dict(os.environ, XDG_RUNTIME_DIR=runtime)).strip()
            appearance['org.gnome.desktop.interface'][key] = value
    with (root / 'system/desktop-interface.ini').open('w') as output:
        appearance.write(output)

    state_file = Path(os.environ.get('XDG_STATE_HOME') or home / '.local/state') / 'wallpaper-studio/state.json'
    state = json.loads(state_file.read_text()) if state_file.is_file() else {}
    sources = [home / 'wallpapers', home / 'imagenes/wallpapers']
    files = [file for folder in sources if folder.is_dir() for file in folder.rglob('*')
             if file.is_file() and file.suffix.lower() in {'.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.tif', '.tiff'}]
    current = Path(state['current']) if state.get('current') else None
    if current and current.is_file() and current not in files:
        files.append(current)
    names = {}
    for file in files:
        if file.name in names and file.read_bytes() != names[file.name].read_bytes():
            raise ValueError('Dos fondos diferentes comparten el nombre ' + file.name)
        names[file.name] = file
        copy_file(file, root / 'wallpapers' / file.name)
    if state:
        state['folders'] = ['@HOME@/wallpapers']
        state['palettes'] = {'@HOME@/wallpapers/' + Path(path).name: colors
                             for path, colors in state.get('palettes', {}).items() if Path(path).name in names}
        state['current'] = '@HOME@/wallpapers/' + current.name if current else None
        target = root / 'state/wallpaper-studio/state.json'
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n')
        lock = root / 'hypr/hyprlock.conf'
        text = lock.read_text()
        if current:
            text = re.sub(r'(?m)^\$ws_wallpaper = .*$', lambda _: '$ws_wallpaper = @HOME@/wallpapers/' + current.name, text)
        lock.write_text(text)
    waypaper = root / 'waypaper/config.ini'
    if waypaper.exists():
        settings = configparser.ConfigParser(interpolation=None)
        settings.read(waypaper)
        settings['Settings']['folder'] = '~/wallpapers'
        if current:
            settings['Settings']['wallpaper'] = '~/wallpapers/' + current.name
        with waypaper.open('w') as output:
            settings.write(output)
    bar = root / 'waybar/config.jsonc'
    if bar.is_file():
        bar.write_text(bar.read_text().replace('launch-in-workspace.py waypaper', 'launch-in-workspace.py wallpaper-studio'))
    for name in ('monitor-handler-dynamic.sh', 'monitor-switch.sh'):
        script = root / 'hypr' / name
        if script.is_file():
            script.write_text(script.read_text().replace('  waypaper --restore', '  wallpaper-studio --restore || waypaper --restore'))

    local = root / 'local'
    launcher = home / '.local/bin/artcraft-launcher'
    desktop = home / '.local/share/applications/ai.storyteller.artcraft-launcher.desktop'
    for file in (launcher, desktop):
        if file.is_file():
            copy_file(file, local / file.relative_to(home / '.local'))
    for file in root.rglob('*'):
        if not file.is_file() or file.is_symlink() or set(file.relative_to(root).parts) & {'apps', 'scripts', 'tests', 'inventory', 'target'}:
            continue
        with file.open('rb') as source:
            executable = source.read(4) == b'\x7fELF'
        if executable:
            omitted.append(str(file.relative_to(root)))
            file.unlink()
            continue
        try:
            text = file.read_text()
        except UnicodeError:
            continue
        private = file.relative_to(root) == Path('eww/os') or any(pattern.search(text) for pattern in SECRET_PATTERNS)
        if file.suffix in ('.json', '.jsonc'):
            private = private or has_credentials(jsonc(text))
        if private:
            omitted.append(str(file.relative_to(root)))
            file.unlink()
    report = {'captured_utc': datetime.now(timezone.utc).isoformat(),
              'official_packages': len(official), 'aur_packages': len(aur),
              'wallpapers': len(names), 'omitted_private_or_compiled': sorted(omitted)}
    (inventory / 'capture.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(f'Notebook: {len(official)} paquetes oficiales, {report["aur_packages"]} AUR, {len(names)} fondos; {len(omitted)} archivos privados/compilados omitidos.')


if __name__ == '__main__':
    capture(Path(sys.argv[1]), Path.home())

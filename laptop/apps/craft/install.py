#!/usr/bin/env python3
"""Restore the notebook's Craft releases from verified upstream artifacts."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.parse
import urllib.request


def download(asset, directory):
    url = urllib.parse.urlsplit(asset['url'])
    if url.scheme != 'https' or url.hostname != 'github.com' or not url.path.startswith('/storytold/') or url.query or url.fragment or url.username:
        raise ValueError('El artefacto no pertenece a los releases oficiales esperados')
    if not re.fullmatch('[0-9a-f]{64}', asset['sha256']):
        raise ValueError('SHA-256 inválido')
    file = directory / Path(url.path).name
    request = urllib.request.Request(asset['url'], headers={'User-Agent': 'arch-workstation-notebook'})
    with urllib.request.urlopen(request, timeout=60) as response, file.open('wb') as output:
        shutil.copyfileobj(response, output)
    with file.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    if digest != asset['sha256']:
        raise ValueError('El SHA-256 descargado no coincide: ' + file.name)
    return file


def install(assets, home):
    subprocess.run(['flatpak', 'remote-add', '--user', '--if-not-exists', 'flathub',
                    'https://dl.flathub.org/repo/flathub.flatpakrepo'], check=True)
    with tempfile.TemporaryDirectory(prefix='notebook-craft-') as temporary:
        for asset in assets:
            if asset['kind'] == 'flatpak':
                current = subprocess.run(['flatpak', 'info', '--user', '--show-commit', asset['id']],
                                         capture_output=True, text=True)
                if current.returncode == 0 and current.stdout.strip() == asset['installed_commit']:
                    continue
                file = download(asset, Path(temporary))
                subprocess.run(['flatpak', 'install', '--user', '--noninteractive', str(file)], check=True)
            elif asset['kind'] == 'appimage' and asset['id'] == 'artcraft-launcher':
                target = home / '.local/opt/artcraft-launcher.AppImage'
                if target.is_file():
                    with target.open('rb') as current:
                        if hashlib.file_digest(current, 'sha256').hexdigest() == asset['sha256']:
                            continue
                file = download(asset, Path(temporary))
                target.parent.mkdir(parents=True, exist_ok=True)
                fd, stage = tempfile.mkstemp(prefix='.artcraft-', dir=target.parent)
                try:
                    with os.fdopen(fd, 'wb') as output, file.open('rb') as source:
                        shutil.copyfileobj(source, output)
                        os.fchmod(output.fileno(), 0o755)
                    os.replace(stage, target)
                finally:
                    Path(stage).unlink(missing_ok=True)
            else:
                raise ValueError('Tipo de aplicación no admitido')


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    install(json.loads((root / 'packages/craft-releases.json').read_text()), Path.home())

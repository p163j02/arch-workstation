#!/usr/bin/env python3
"""Read-only checks for the distributable dotfiles and current package names."""
import argparse
import ast
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.parse
import urllib.request

from portable import transform

ROOT = Path(__file__).resolve().parents[1]
SKIP = {'.git', 'node_modules', '__pycache__', 'graphify-out', 'target'}
SENSITIVE_KEYS = re.compile(r'^(password|passwd|token|secret|bearer|credentials?|api[_-]?key|api[_-]?token|access[_-]?token|refresh[_-]?token|client[_-]?secret|authorization|private[_-]?key)$', re.I)
SECRET_PATTERNS = [
    re.compile(r'-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----'),
    re.compile(r'\bgh[pousr]_[A-Za-z0-9]{20,}\b'),
    re.compile(r'\bgithub_pat_[A-Za-z0-9_]{20,}\b'),
    re.compile(r'\bAKIA[A-Z0-9]{16}\b'),
    re.compile(r'\bsk-(?:proj-|ant-)?[A-Za-z0-9_-]{24,}\b'),
]


def files(root, include_private=False):
    for base, dirs, names in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if d not in SKIP]
        for name in names:
            if include_private or not re.search(r'\.(bak|save|local)\b|\.pyc$', name):
                yield Path(base) / name


def jsonc(text):
    pattern = r'("(?:\\.|[^"\\])*")|//[^\n]*|/\*.*?\*/|,(?=\s*[}\]])'
    return json.loads(re.sub(pattern, lambda m: m[1] or '', text, flags=re.S))


def reject_credentials(value, path):
    if isinstance(value, dict):
        for key, item in value.items():
            if SENSITIVE_KEYS.fullmatch(key) and isinstance(item, str) and item:
                if not re.fullmatch(r'\{env:[A-Z_][A-Z0-9_]*\}', item):
                    raise ValueError(f'Credencial literal: {path}')
            reject_credentials(item, path)
    elif isinstance(value, list):
        for item in value:
            reject_credentials(item, path)


def check_secrets(root):
    forbidden = {'service.json', 'auth.json', 'fish_variables', 'id_rsa', 'id_ed25519', 'local.conf', 'local.lua'}
    for path in files(root, include_private=True):
        if path.is_symlink():
            continue
        relative = path.relative_to(root)
        if path.name in forbidden or path.suffix in ('.pem', '.key', '.p12', '.pfx', '.keystore'):
            raise ValueError(f'Archivo privado no permitido: {relative}')
        if path.name.startswith(('.env', '.credentials')) or re.search(r'\.(local|bak|save)\b', path.name):
            raise ValueError(f'Archivo de entorno privado no permitido: {relative}')
        try:
            text = path.read_text()
        except UnicodeError:
            continue
        if any(pattern.search(text) for pattern in SECRET_PATTERNS):
            raise ValueError(f'Posible secreto: {relative}; valor omitido')
        if path.suffix in ('.json', '.jsonc'):
            reject_credentials(jsonc(text), relative)
    print('Comprobación de credenciales: sin hallazgos.')


def package_names(root):
    return sorted({line.split('#')[0].strip()
                   for p in root.glob('*/packages/official.txt')
                   for line in p.read_text().splitlines() if line.split('#')[0].strip()})


def check_packages(root):
    available = set(subprocess.check_output(['pacman', '-Slq'], text=True).split())
    hardware = ['amd-ucode', 'intel-ucode', 'nvidia-open-dkms', 'nvidia-utils',
                'lib32-nvidia-utils', 'nvidia-settings', 'mesa', 'lib32-mesa',
                'vulkan-radeon', 'lib32-vulkan-radeon', 'vulkan-intel', 'lib32-vulkan-intel']
    missing = sorted(set(package_names(root) + hardware) - available)
    if missing:
        raise ValueError('Paquetes oficiales ausentes: ' + ', '.join(missing) + '. Actualiza las bases con pacman -Syu.')
    aur = sorted({l.split('#')[0].strip() for p in root.glob('*/packages/aur.txt')
                  for l in p.read_text().splitlines() if l.split('#')[0].strip()})
    query = urllib.parse.urlencode([('arg[]', n) for n in aur])
    with urllib.request.urlopen('https://aur.archlinux.org/rpc/v5/info?' + query, timeout=45) as response:
        data = json.load(response)
    if data.get('type') != 'multiinfo':
        raise ValueError('Respuesta inesperada de AUR')
    found = {p['Name']: p for p in data['results']}
    missing = sorted(set(aur) - found.keys())
    if missing:
        raise ValueError('Paquetes AUR ausentes: ' + ', '.join(missing))
    for name, p in found.items():
        if p.get('OutOfDate') or not p.get('Maintainer'):
            print('Aviso AUR: revisar mantenimiento de ' + name)
    print(f'Paquetes: {len(package_names(root))} oficiales y {len(aur)} AUR disponibles.')


def check(root):
    check_secrets(root)
    counts = dict(bash=0, fish=0, python=0, json=0)
    for path in files(root):
        if path.is_symlink():
            target = Path(os.readlink(path))
            if not target.is_absolute() and not path.exists():
                raise ValueError(f'Enlace roto: {path.relative_to(root)}')
            continue
        try:
            text = path.read_text()
        except UnicodeError:
            continue
        if '\0' in text or not text:
            continue
        if (text.startswith('#!') and 'python' in text.splitlines()[0]) or path.suffix == '.py':
            ast.parse(text, filename=str(path))
            counts['python'] += 1
        elif path.suffix in ('.sh', '.fish'):
            interpreter = 'fish' if path.suffix == '.fish' else 'bash'
            subprocess.run([interpreter, '-n', str(path)], check=True, capture_output=True, text=True)
            counts[interpreter] += 1
        elif path.suffix == '.lua':
            subprocess.run(['luac', '-p', str(path)], check=True, capture_output=True, text=True)
        if path.suffix in ('.json', '.jsonc'):
            jsonc(text)
            counts['json'] += 1
        if '/home/' + 'p163j0' in text:
            raise ValueError(f'Ruta personal sin normalizar: {path.relative_to(root)}')

    hyprland = shutil.which('Hyprland')
    if not hyprland:
        raise ValueError('Falta Hyprland para validar la configuración. Instálalo antes de --check.')
    for profile in ('laptop', 'desktop'):
        with tempfile.TemporaryDirectory(prefix='dotfiles-check-') as temp:
            stage = Path(temp) / '.config/hypr'
            shutil.copytree(root / profile / 'hypr', stage)
            (stage / 'local.conf').touch()
            transform(stage, temp, render=True)
            config = stage / 'hyprland.lua'
            # Validate the assembled sources without reading the active user's Hyprland config.
            config.write_text(config.read_text().replace('~/.config/hypr/', str(stage) + '/'))
            result = subprocess.run([hyprland, '--verify-config', '-c', str(config)],
                                    capture_output=True, text=True, cwd=temp)
            if result.returncode or 'config ok' not in result.stdout:
                raise ValueError(f'Hyprland {profile}: {result.stdout}\n{result.stderr}')
    print('Sintaxis y configuración de ambos perfiles: OK. ' + str(counts))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--packages', action='store_true')
    parser.add_argument('--secrets-only', action='store_true')
    args = parser.parse_args()
    try:
        if args.secrets_only:
            check_secrets(args.root)
        else:
            check(args.root)
        if args.packages:
            check_packages(args.root)
    except (OSError, ValueError, SyntaxError, subprocess.SubprocessError) as error:
        raise SystemExit('Comprobación fallida: ' + str(error))

import hashlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('craft_install', ROOT / 'apps/craft/install.py')
craft = importlib.util.module_from_spec(spec)
spec.loader.exec_module(craft)


class CraftTests(unittest.TestCase):
    def asset(self, data=b'release'):
        return dict(id='artcraft-launcher', kind='appimage',
                    url='https://github.com/storytold/craft-launcher/releases/download/v1/test.AppImage',
                    sha256=hashlib.sha256(data).hexdigest())

    def test_rejects_unexpected_download_source(self):
        asset = self.asset()
        asset['url'] = 'https://example.org/application'
        with tempfile.TemporaryDirectory() as directory, self.assertRaises(ValueError):
            craft.download(asset, Path(directory))

    def test_failed_hash_preserves_existing_appimage(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            target = home / '.local/opt/artcraft-launcher.AppImage'
            target.parent.mkdir(parents=True)
            target.write_bytes(b'existing')
            with patch.object(craft.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)), \
                 patch.object(craft.urllib.request, 'urlopen', return_value=io.BytesIO(b'corrupted')):
                with self.assertRaises(ValueError):
                    craft.install([self.asset()], home)
            self.assertEqual(target.read_bytes(), b'existing')

    def test_installs_verified_appimage_atomically(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            with patch.object(craft.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)), \
                 patch.object(craft.urllib.request, 'urlopen', return_value=io.BytesIO(b'release')):
                craft.install([self.asset()], home)
            target = home / '.local/opt/artcraft-launcher.AppImage'
            self.assertEqual(target.read_bytes(), b'release')
            self.assertEqual(target.stat().st_mode & 0o777, 0o755)

    def test_matching_flatpak_commit_skips_download(self):
        asset = self.asset()
        asset.update(kind='flatpak', id='ai.storyteller.gridcraft', installed_commit='known-commit')
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(craft.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'known-commit\n')), \
             patch.object(craft.urllib.request, 'urlopen') as download:
            craft.install([asset], Path(directory))
            download.assert_not_called()

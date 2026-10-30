"""Exercise the app installer in an isolated HOME without recompiling or applying settings."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class SettingsInstallTests(unittest.TestCase):
    def test_installer_preserves_preferences_and_installs_apps_and_gallery(self):
        with tempfile.TemporaryDirectory(prefix='notebook-settings-install-') as directory:
            workspace = Path(directory)
            profile, home = workspace / 'notebook', workspace / 'home'
            app = profile / 'apps/waybar-editor'
            app.mkdir(parents=True)
            shutil.copy2(ROOT / 'apps/waybar-editor/install.sh', app / 'install.sh')
            shutil.copytree(ROOT / 'fastfetch/images', profile / 'fastfetch/images')
            executable = app / 'target/release/waybar-editor'
            executable.parent.mkdir(parents=True)
            executable.write_text('#!/bin/sh\nexit 0\n')
            executable.chmod(0o755)
            binaries = workspace / 'bin'
            binaries.mkdir()
            cargo = binaries / 'cargo'
            cargo.write_text('#!/bin/sh\nexit 0\n')
            cargo.chmod(0o755)
            config = home / '.config'
            fastfetch = config / 'fastfetch/config.jsonc'
            fastfetch.parent.mkdir(parents=True)
            fastfetch.write_text('{"modules":["battery"]}\n')
            state = home / '.local/state/wallpaper-studio/state.json'
            state.parent.mkdir(parents=True)
            state.write_text('{"current":null,"palettes":{}}\n')
            env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(config),
                       XDG_DATA_HOME=str(home / '.local/share'),
                       XDG_STATE_HOME=str(home / '.local/state'),
                       PATH=str(binaries) + os.pathsep + os.environ['PATH'])
            result = subprocess.run(['sh', str(app / 'install.sh')], env=env,
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(fastfetch.read_text(), '{"modules":["battery"]}\n')
            self.assertEqual(state.read_text(), '{"current":null,"palettes":{}}\n')
            self.assertFalse((config / 'pc-settings/widgets.json').exists())
            for name in ('wallpaper-studio', 'pc-settings'):
                self.assertEqual((home / '.local/bin' / name).resolve(),
                                 home / '.local/bin/waybar-editor')
            for name in ('waybar-editor', 'wallpaper-studio', 'pc-settings'):
                entry = home / '.local/share/applications' / f'{name}.desktop'
                self.assertTrue(entry.is_file(), name)
                self.assertIn(str(home / '.local/bin' / name), entry.read_text())
            self.assertIn('Name=Configuración del notebook',
                          (home / '.local/share/applications/pc-settings.desktop').read_text())
            source = profile / 'fastfetch/images'
            images = list(source.glob('wallpaper-studio-*.png')) + list(source.glob('silphwave/*/*.png'))
            self.assertEqual(len(images), 23)
            for image in images:
                installed = config / 'fastfetch/images' / image.relative_to(source)
                self.assertEqual(installed.read_bytes(), image.read_bytes(), str(image))


if __name__ == '__main__':
    unittest.main()

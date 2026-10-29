import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class IsolationTests(unittest.TestCase):
    def test_standalone_install_preserves_private_data_and_existing_state(self):
        with tempfile.TemporaryDirectory(prefix='laptop-isolation-') as directory:
            workspace = Path(directory)
            profile, home = workspace / 'notebook', workspace / 'home'
            shutil.copytree(ROOT, profile, symlinks=True,
                            ignore=shutil.ignore_patterns('target', '__pycache__', 'graphify-out', '.git'))
            home.mkdir()
            runtime = home / 'run'
            runtime.mkdir()
            cache = home / '.config/Code/User/globalStorage/keep.dat'
            cache.parent.mkdir(parents=True)
            cache.write_text('private runtime data')
            env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / '.config'),
                       XDG_STATE_HOME=str(home / '.local/state'), XDG_DATA_HOME=str(home / '.local/share'),
                       XDG_RUNTIME_DIR=str(runtime), PYTHONDONTWRITEBYTECODE='1')
            env.pop('DBUS_SESSION_BUS_ADDRESS', None)

            def install():
                result = subprocess.run(['bash', str(profile / 'install.sh'), '--config-only'],
                                        cwd=workspace, env=env, input='si\n', capture_output=True,
                                        text=True, timeout=45)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

            install()
            self.assertFalse((workspace / 'desktop').exists())
            self.assertEqual((home / '.config/dotfiles/profile').read_text(), 'laptop\n')
            self.assertTrue((home / '.config/hypr/session-start.sh').is_file())
            self.assertIn('wallpaper-studio --restore', (home / '.config/hypr/session-start.sh').read_text())
            self.assertTrue((home / '.config/hypr/hyprland.lua').is_file())
            self.assertEqual(cache.read_text(), 'private runtime data')
            state = home / '.local/state/wallpaper-studio/state.json'
            self.assertTrue(state.is_file())
            self.assertNotIn('@HOME@', state.read_text())
            self.assertEqual(state.stat().st_mode & 0o777, 0o600)
            state.write_text('{"current":null,"palettes":{}}\n')
            install()
            self.assertEqual(state.read_text(), '{"current":null,"palettes":{}}\n')
            self.assertEqual(cache.read_text(), 'private runtime data')
            (home / '.config/dotfiles/profile').write_text('desktop\n')
            result = subprocess.run(['bash', str(profile / 'update.sh')], env=env,
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Este equipo usa desktop', result.stderr)

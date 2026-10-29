import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('capture_notebook', ROOT / 'scripts/capture-extra.py')
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


class CaptureTests(unittest.TestCase):
    def test_capture_keeps_portable_state_and_omits_private_preferences(self):
        with tempfile.TemporaryDirectory() as directory:
            home, target = Path(directory) / 'home', Path(directory) / 'profile'
            for name in ('packages', 'system', 'hypr'):
                (target / name).mkdir(parents=True)
            (target / 'packages/official.txt').write_text('git\nrust\n')
            (target / 'hypr/hyprlock.conf').write_text('$ws_wallpaper = previous.png\n')
            settings = home / '.config/Code/User/settings.json'
            settings.parent.mkdir(parents=True)
            settings.write_text(json.dumps({'apiKey': 'private-placeholder'}))
            image = home / 'imagenes/wallpapers/test.png'
            image.parent.mkdir(parents=True)
            image.write_bytes(b'test-image')
            state = home / '.local/state/wallpaper-studio/state.json'
            state.parent.mkdir(parents=True)
            state.write_text(json.dumps({'current': str(image), 'palettes': {str(image): {'accent': '#123456'}}}))

            def command(args, **_kwargs):
                if args[0] == 'gsettings':
                    return "'theme'\n"
                return {'-Qqen': 'git\n', '-Qqn': 'git\nrust\n',
                        '-Qqem': 'eww\nyay\nclaude-desktop-bin\n', '-Q': 'git 1\nrust 1\n'}[args[1]]

            with patch.dict(os.environ, XDG_CONFIG_HOME=str(home / '.config'), XDG_STATE_HOME=str(home / '.local/state')), \
                 patch.object(capture.subprocess, 'check_output', side_effect=command), \
                 patch.object(capture.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)), \
                 patch.object(capture.shutil, 'which', return_value=None):
                capture.capture(target, home)
            saved = json.loads((target / 'state/wallpaper-studio/state.json').read_text())
            self.assertEqual(saved['current'], '@HOME@/wallpapers/test.png')
            self.assertEqual((target / 'wallpapers/test.png').read_bytes(), b'test-image')
            self.assertFalse((target / 'preferences/Code/User/settings.json').exists())
            self.assertEqual((target / 'packages/aur.txt').read_text(), 'claude-desktop\neww\n')
            self.assertTrue(settings.exists())
            self.assertEqual(json.loads((target / 'inventory/capture.json').read_text())['omitted_private_or_compiled'],
                             ['preferences/Code/User/settings.json'])

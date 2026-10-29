"""Pixie palette integration preserves existing local theme customizations."""
import importlib.util
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('sddm_theme', ROOT / 'apps/wallpaper-studio/sddm_theme.py')
theme = importlib.util.module_from_spec(spec)
spec.loader.exec_module(theme)


class LoginThemeTests(unittest.TestCase):
    @unittest.skipIf(os.geteuid() == 0, 'The installer must run as a normal user')
    def test_setup_rejects_old_binary_before_changing_files(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            binary = home / '.local/bin/wallpaper-studio'
            binary.parent.mkdir(parents=True)
            binary.write_text('#!/bin/sh\nprintf "%s\\n" "wallpaper-studio --restore"\n')
            binary.chmod(0o755)
            state = home / 'state'
            result = subprocess.run(['bash', str(ROOT / 'apps/wallpaper-studio/setup-login-theme.sh')],
                                    env=dict(os.environ, HOME=str(home), XDG_STATE_HOME=str(state)),
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Actualiza primero Wallpaper Studio', result.stderr)
            self.assertFalse(state.exists())

    def test_palette_bindings_preserve_local_text_and_are_idempotent(self):
        main = 'property color extractedAccent: config.accentColor\ncolor: "white"\ntext: "Introduce tu contraseña"'
        clock = 'function updateColors() {\n    var base = clock.baseAccent;\n}'
        themed_main, themed_clock = theme.themed_sources(main, clock)
        self.assertIn('extractedAccent: config.primaryColor', themed_main)
        self.assertIn('color: config.textColor', themed_main)
        self.assertIn('Introduce tu contraseña', themed_main)
        self.assertIn('smartMinutesColor = config.accentColor;', themed_clock)
        self.assertEqual(theme.themed_sources(themed_main, themed_clock), (themed_main, themed_clock))

    def test_rejects_an_unexpected_theme(self):
        with self.assertRaises(ValueError):
            theme.themed_sources('unrelated theme', 'unrelated clock')

    def test_repository_theme_is_already_bound(self):
        pixie = ROOT / 'sddm/pixie'
        sources = ((pixie / 'Main.qml').read_text(), (pixie / 'components/Clock.qml').read_text())
        self.assertEqual(theme.themed_sources(*sources), sources)


if __name__ == '__main__':
    unittest.main()

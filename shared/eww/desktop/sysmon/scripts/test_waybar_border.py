#!/usr/bin/env python3
"""Offline parser and live file-watch tests; never modify real Waybar settings."""
import importlib.util
import json
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('waybar-border.py')
SPEC = importlib.util.spec_from_file_location('waybar_border', SCRIPT)
BORDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BORDER)


def fixture(color='#65d9ff', shadow='0 0 6px rgba(101, 217, 255, 0.3), inset 0 0 4px rgba(101, 217, 255, 0.3)'):
    return ('window#waybar { border: 9px solid #ff0000; }\n'
            '/* waybar-editor:begin */\n/* waybar-editor:appearance {} */\n'
            'window#waybar .modules-left, window#waybar .modules-center, window#waybar .modules-right {'
            f'background: #ffffff; border: 1px solid {color}; border-radius: 20px; box-shadow: {shadow};'
            '}\n/* waybar-editor:end */')


class BorderTests(unittest.TestCase):
    def test_effective_css_only_and_no_background_leak(self):
        style = BORDER.contour(fixture())
        self.assertIn('border: 1px solid #65d9ff;', style)
        self.assertNotIn('#ff0000', style)
        self.assertNotIn('background', style)

    def test_no_glow(self):
        self.assertIn('box-shadow: none;', BORDER.contour(fixture(shadow='none')))

    def test_rgba_and_geometry(self):
        text = fixture('rgba(64, 191, 191, 0.500)').replace('border: 1px', 'border: 2.5px').replace('20px', '18px')
        style = BORDER.contour(text)
        self.assertIn('border: 2.5px solid rgba(64, 191, 191, 0.500)', style)
        self.assertIn('border-radius: 18px', style)

    def test_incomplete_block_and_json_rejected(self):
        for text in [fixture().replace('/* waybar-editor:end */', ''), fixture().replace('appearance {}', 'appearance {')]:
            with self.assertRaises(ValueError):
                BORDER.contour(text)

    def test_invalid_values_rejected(self):
        for text in [fixture('rgba(999, 0, 0, 1)'), fixture('rgba(64, 191, 191, 2)'),
                     fixture().replace('20px', '-1px'), fixture(shadow='url(unsafe)'),
                     fixture().replace('border: 1px', 'border: 100px')]:
            with self.assertRaises(ValueError):
                BORDER.contour(text)

    def test_live_atomic_changes_last_valid_and_no_duplicate(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'style.css'
            path.write_text(fixture())
            process = subprocess.Popen([sys.executable, str(SCRIPT), str(path)], stdout=subprocess.PIPE, text=True)
            def sample():
                self.assertTrue(select.select([process.stdout], [], [], 3)[0], 'Listener did not update')
                return json.loads(process.stdout.readline())['style']
            try:
                self.assertEqual(sample(), BORDER.contour(fixture()))
                path.write_text('incomplete')
                self.assertFalse(select.select([process.stdout], [], [], .8)[0])
                replacement = Path(directory) / 'new.css'
                replacement.write_text(fixture('#ff925e', 'none'))
                replacement.replace(path)
                self.assertEqual(sample(), BORDER.contour(fixture('#ff925e', 'none')))
                path.write_text(fixture('#ff925e', 'none'))
                self.assertFalse(select.select([process.stdout], [], [], .8)[0])
                path.unlink()
                self.assertFalse(select.select([process.stdout], [], [], .8)[0])
                path.write_text(fixture())
                self.assertEqual(sample(), BORDER.contour(fixture()))
            finally:
                process.terminate()
                process.wait(timeout=3)
                process.stdout.close()
            self.assertEqual(process.returncode, 0)

    def test_missing_file_initial_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(next(BORDER.styles(Path(directory) / 'missing.css')), BORDER.FALLBACK)


if __name__ == '__main__':
    unittest.main()

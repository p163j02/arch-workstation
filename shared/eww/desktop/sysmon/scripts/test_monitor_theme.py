"""Palette and chart synchronization tests, using only temporary Waybar copies."""
from pathlib import Path
import json
import select
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

from monitor_theme import Theme, palette, wallpaper_colors
from test_waybar_border import BORDER, fixture
from test_cpu_activity import CPU
from test_resource_activity import RESOURCE


class ThemeTests(unittest.TestCase):
    def test_exact_accent_for_supported_color_formats(self):
        for color, expected in [('#abc', '#aabbcc'), ('#F178AB', '#f178ab'),
                                ('rgb(64, 191, 191)', '#40bfbf'),
                                ('rgba(64, 191, 191, .5)', '#40bfbf')]:
            tokens = palette(BORDER.contour(fixture(color)))
            self.assertEqual(tokens['primary'], expected)
            self.assertEqual(tokens['style'], BORDER.contour(fixture(color)))

    def test_theme_retains_last_valid_and_detects_atomic_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'style.css'
            path.write_text(fixture('#8dccf4'))
            theme = Theme(path)
            nord = theme.read()
            path.write_text('incomplete')
            self.assertEqual(theme.read(), nord)
            new = Path(directory) / 'new.css'
            new.write_text(fixture('#f178ab'))
            new.replace(path)
            self.assertEqual(theme.read()['primary'], '#f178ab')
            path.unlink()
            self.assertEqual(theme.read()['primary'], '#f178ab')

    def test_missing_css_fallback_and_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'missing.css'
            theme = Theme(path)
            self.assertEqual(theme.read()['primary'], '#40bfbf')
            path.write_text(fixture('#25e0c1'))
            self.assertEqual(theme.read()['primary'], '#25e0c1')

    def test_charts_use_same_theme_without_changing_samples(self):
        ns = {'svg': 'http://www.w3.org/2000/svg'}
        tokens = palette(BORDER.contour(fixture('#f178ab')))
        with patch.object(CPU, 'chart_colors', return_value=tokens), patch.object(RESOURCE, 'chart_colors', return_value=tokens):
            charts = [CPU.chart([0, 50, 100])[0], RESOURCE.disk_chart([0, 1024**2, 2 * 1024**2])[0]]
            for svg in charts:
                root = ET.fromstring(svg)
                self.assertEqual(root.find('svg:polyline', ns).attrib['stroke'], '#f178ab')
                self.assertEqual(len(root.find('svg:polyline', ns).attrib['points'].split()), 3)

    def test_text_contrast_on_derived_dark_surfaces(self):
        def luminance(hex_color):
            rgb = [int(hex_color[i:i+2], 16) / 255 for i in (1, 3, 5)]
            linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in rgb]
            return sum(v * w for v, w in zip(linear, (.2126, .7152, .0722)))
        for accent in ('#25e0c1', '#8dccf4', '#f178ab', '#40bfbf', '#000000', '#ffffff'):
            tokens = palette(BORDER.contour(fixture(accent)))
            for foreground in ('text', 'muted', 'soft'):
                for background in ('background', 'card'):
                    ratio = (luminance(tokens[foreground]) + .05) / (luminance(tokens[background]) + .05)
                    self.assertGreaterEqual(ratio, 4.5, (accent, foreground, background))


    @staticmethod
    def studio_css(**changes):
        colors = dict(accent='#65d9ff', secondary='#bb9af7', background='#10131d', foreground='#f2f4ff')
        colors.update(changes)
        return ('/* wallpaper-studio:begin */\n/* wallpaper-studio:colors '
                + json.dumps(colors) + ' */\n/* wallpaper-studio:end */')

    def test_studio_palette_updates_widget_and_charts_without_editor_block(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'style.css'
            path.write_text(self.studio_css())
            theme = Theme(path)
            tokens = theme.read()
            self.assertEqual(tokens['primary'], '#65d9ff')
            self.assertEqual(tokens['soft'], '#bb9af7')
            self.assertEqual(tokens['background'], '#10131d')
            self.assertEqual(tokens['text'], '#f2f4ff')
            BORDER.valid_shadow(tokens['style'].split('box-shadow: ')[1].rstrip(';'))
            path.write_text(self.studio_css(foreground='#eeeeee', background='#222233'))
            changed = theme.read()
            self.assertEqual(changed['primary'], tokens['primary'])
            self.assertEqual(changed['text'], '#eeeeee')
            self.assertEqual(changed['background'], '#222233')
            self.assertNotEqual(changed['card'], tokens['card'])

    def test_studio_validates_complete_palette_and_keeps_last_good(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'style.css'
            path.write_text(fixture() + self.studio_css())
            theme = Theme(path)
            expected = theme.read()
            for broken in [self.studio_css(accent='url(invalid)'), self.studio_css(foreground=None),
                           self.studio_css().replace('/* wallpaper-studio:end */', '')]:
                path.write_text(fixture() + broken)
                self.assertEqual(theme.read(), expected)
            self.assertIsNone(wallpaper_colors('/* wallpaper-studio:colors {} */'))

    def test_live_listener_emits_when_only_background_or_text_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'style.css'
            path.write_text(fixture() + self.studio_css())
            process = subprocess.Popen([sys.executable, str(Path(__file__).with_name('waybar-border.py')), str(path)], stdout=subprocess.PIPE, text=True)
            def sample():
                self.assertTrue(select.select([process.stdout], [], [], 3)[0], 'Palette update not emitted')
                return json.loads(process.stdout.readline())
            try:
                before = sample()
                path.write_text(fixture() + self.studio_css(background='#222233', foreground='#eeeeee'))
                after = sample()
                self.assertEqual(before['style'], after['style'])
                self.assertEqual(after['background'], '#222233')
                self.assertEqual(after['text'], '#eeeeee')
            finally:
                process.terminate()
                process.wait(timeout=3)
                process.stdout.close()


if __name__ == '__main__':
    unittest.main()

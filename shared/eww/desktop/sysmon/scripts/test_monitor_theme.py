"""Palette and chart synchronization tests, using only temporary Waybar copies."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

from monitor_theme import Theme, palette
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


if __name__ == '__main__':
    unittest.main()

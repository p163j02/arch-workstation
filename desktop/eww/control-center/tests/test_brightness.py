"""Backlight and desktop DDC tests; no real monitor commands are executed."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import backend


class BrightnessTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.run = self.root / 'runtime'
        self.drm = self.root / 'drm'
        self.backlight = self.root / 'backlight'
        for path in (self.run, self.drm, self.backlight):
            path.mkdir()
        self.cfg = dict(backend.config(), external_connector='', ddc_bus='')
        for target, value in [('RUN', self.run), ('DRM_ROOT', self.drm),
                              ('BACKLIGHT_ROOT', self.backlight), ('config', lambda: self.cfg)]:
            mock = patch.object(backend, target, value)
            mock.start()
            self.addCleanup(mock.stop)

    def monitor(self, connector='DP-3', edid=b'fake-edid'):
        path = self.drm / ('card1-' + connector)
        path.mkdir()
        (path / 'status').write_text('connected')
        (path / 'edid').write_bytes(edid)
        return path

    def internal(self):
        path = self.backlight / 'intel_backlight'
        path.mkdir()
        (path / 'max_brightness').write_text('1000')
        (path / 'brightness').write_text('420')
        return path

    @staticmethod
    def external(**changes):
        state = dict(available=True, percent=65, bus='7', maximum=100,
                     name='DP-3', message='DDC/CI conectado', identity=['test'])
        state.update(changes)
        return state

    def test_desktop_slider_uses_external_monitor_without_notebook(self):
        with patch.object(backend, 'external_brightness', return_value=self.external()):
            state = backend.brightness_state()
        self.assertTrue(state['available'])
        self.assertFalse(state['internal']['available'])
        self.assertEqual(state['percent'], 65)
        self.assertEqual(state['device'], 'DP-3')

    def test_desktop_write_does_not_require_brightnessctl_or_backlight(self):
        with patch.object(backend, 'external_brightness', return_value=self.external()), \
             patch.object(backend, 'command', return_value='') as command:
            backend.set_displays(37)
        command.assert_called_once_with(['ddcutil', '--bus', '7', 'setvcp', '10', '37'], timeout=5)
        saved = json.loads((self.run / 'ddc-state.json').read_text())
        self.assertEqual(saved['state']['percent'], 37)
        self.assertEqual(saved['identity'], ['test'])

    def test_nvidia_detection_without_sysfs_ddc_link(self):
        self.monitor()
        raw = 'Display 1\n I2C bus: /dev/i2c-7\n DRM_connector: card1-DP-3\n Monitor: Test\n'
        with patch.object(backend.shutil, 'which', return_value='/usr/bin/ddcutil'), \
             patch.object(backend.os.path, 'exists', return_value=True), \
             patch.object(backend.os, 'access', return_value=True), \
             patch.object(backend, 'command', side_effect=[raw, 'VCP 10 C 70 200']) as command:
            state = backend.external_brightness()
        self.assertTrue(state['available'], state)
        self.assertEqual(state['percent'], 35)
        self.assertEqual(command.call_args_list[0].args[0], ['ddcutil', 'detect', '--brief'])
        self.assertEqual(command.call_args_list[1].args[0], ['ddcutil', '--bus', '7', '--brief', 'getvcp', '10'])

    def test_ddc_parser_matches_connector_instead_of_first_display(self):
        display = self.monitor()
        raw = ('Display 1\n I2C bus: /dev/i2c-4\n DRM connector: card1-HDMI-A-1\n'
               'Display 2\n I2C bus: /dev/i2c-7\n DRM connector: card1-DP-3\n')
        self.assertEqual(backend.detected_ddc_bus(raw, display, False), '7')

    def test_legacy_detection_requires_one_unambiguous_monitor(self):
        display = self.monitor()
        raw = 'Display 1\n I2C bus: /dev/i2c-7\n Monitor: TEST\n'
        self.assertEqual(backend.detected_ddc_bus(raw, display, True), '7')
        with self.assertRaisesRegex(RuntimeError, 'ddc_bus'):
            backend.detected_ddc_bus(raw, display, False)
        with self.assertRaisesRegex(RuntimeError, 'ddc_bus'):
            backend.detected_ddc_bus(raw + 'Display 2\n I2C bus: /dev/i2c-8\n', display, True)
        with self.assertRaises(RuntimeError):
            backend.detected_ddc_bus('Invalid display\n I2C bus: /dev/i2c-7\n', display, True)

    def test_missing_i2c_permissions_are_reported_and_slider_remains_unavailable(self):
        self.monitor()
        self.cfg['ddc_bus'] = '7'
        with patch.object(backend.shutil, 'which', return_value='/usr/bin/ddcutil'), \
             patch.object(backend.os.path, 'exists', return_value=True), \
             patch.object(backend.os, 'access', return_value=False), \
             patch.object(backend, 'command') as command:
            state = backend.brightness_state()
        self.assertFalse(state['available'])
        self.assertIn('Sin acceso a /dev/i2c-7', state['message'])
        command.assert_not_called()

    def test_hotplug_invalidates_bus_cache(self):
        display = self.monitor()
        self.cfg['ddc_bus'] = '7'
        with patch.object(backend.shutil, 'which', return_value='/usr/bin/ddcutil'), \
             patch.object(backend.os.path, 'exists', return_value=True), \
             patch.object(backend.os, 'access', return_value=True), \
             patch.object(backend, 'command', return_value='VCP 10 C 65 100') as command:
            self.assertTrue(backend.external_brightness()['available'])
            self.assertTrue(backend.external_brightness()['available'])
            self.assertEqual(command.call_count, 1)
            (display / 'status').write_text('disconnected')
            self.assertFalse(backend.external_brightness()['available'])
            (display / 'status').write_text('connected')
            (display / 'edid').write_bytes(b'other-monitor')
            self.assertTrue(backend.external_brightness()['available'])
            self.assertEqual(command.call_count, 2)

    def test_laptop_remains_usable_without_external_monitor(self):
        self.internal()
        with patch.object(backend.shutil, 'which', return_value='/usr/bin/brightnessctl'), \
             patch.object(backend, 'command', return_value='') as command:
            state = backend.brightness_state()
            backend.set_displays(55)
        self.assertTrue(state['available'])
        self.assertTrue(state['internal']['available'])
        self.assertFalse(state['external']['available'])
        self.assertEqual(state['percent'], 42)
        command.assert_called_once_with(['brightnessctl', '-d', 'intel_backlight', 'set', '55%'])

    def test_external_failure_does_not_prevent_internal_update_or_cache_success(self):
        self.internal()
        def execute(args, **kwargs):
            if args[0] == 'ddcutil':
                raise RuntimeError('Monitor disconnected')
            return ''
        with patch.object(backend, 'external_brightness', return_value=self.external()), \
             patch.object(backend, 'command', side_effect=execute) as command:
            backend.set_displays(40)
        self.assertEqual(command.call_count, 2)
        saved = json.loads((self.run / 'ddc-state.json').read_text())
        self.assertFalse(saved['state']['available'])
        self.assertEqual(saved['state']['percent'], 65)
        self.assertIn('Monitor disconnected', json.loads((self.run / 'brightness-message.json').read_text())['text'])

    def test_cache_write_failure_does_not_hide_hardware_diagnostic(self):
        with patch.object(backend.Path, 'write_text', side_effect=PermissionError('read only')):
            state = backend.external_brightness()
        self.assertFalse(state['available'])
        self.assertIn('No hay un monitor externo', state['message'])

    def test_desktop_slider_action_clamps_and_writes_external(self):
        self.cfg['brightness_min'] = 10
        with patch.object(backend, 'external_brightness', return_value=self.external()), \
             patch.object(backend, 'command', return_value='') as command, \
             patch.object(backend.time, 'sleep'):
            backend.action('brightness', 'set', '2')
        command.assert_called_once_with(['ddcutil', '--bus', '7', 'setvcp', '10', '10'], timeout=5)


if __name__ == '__main__':
    unittest.main()

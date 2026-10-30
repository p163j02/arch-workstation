"""Popup lifetime, replacement and real Lua callbacks without device changes."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('popup_lifetime', ROOT / 'scripts/popup-manager.py')
popup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(popup)


class PopupLifetimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.run = Path(self.temp.name)
        self.patcher = patch.object(popup.backend, 'RUN', self.run)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)

    def state(self, generation='current', module='wifi'):
        (self.run / 'popup.json').write_text(json.dumps(dict(generation=generation, module=module)))

    def test_lua_callbacks(self):
        result = subprocess.run(['lua', str(Path(__file__).with_name('test_popup_input.lua')),
                                 str(ROOT / 'scripts/popup-input.lua')], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_bindings_use_lua_and_only_owned_handles(self):
        self.state()
        with patch.object(popup.backend, 'command', return_value='ok') as command:
            popup.input_bindings(True)
            args = command.call_args.args[0]
            self.assertEqual(args[:2], ['hyprctl', 'eval'])
            self.assertIn('dismiss current', args[2])
            popup.input_bindings(False)
            self.assertIn('install("")', command.call_args.args[0][2])

    def test_delayed_dismiss_cannot_close_replacement(self):
        self.state('second')
        with patch.object(popup, 'close') as close:
            popup.main(['dismiss', 'first'])
            close.assert_not_called()
            popup.main(['dismiss', 'second'])
            close.assert_called_once()

    def test_cleanup_still_closes_windows_when_input_cleanup_fails(self):
        with patch.object(popup, 'input_bindings', side_effect=RuntimeError('compositor unavailable')):
            with patch.object(popup, 'close_windows') as close:
                with self.assertRaises(RuntimeError):
                    popup.close()
                close.assert_called_once()

    def test_bar_click_is_outside_but_panel_click_is_inside(self):
        self.state()
        layers = {'DP-1': {'levels': {'2': [dict(namespace='waybar', x=0, y=0, w=1920, h=40)],
                                     '3': [dict(namespace='waybar-control-center', x=100, y=50, w=380, h=560)]}}}
        for cursor, should_close in [(dict(x=150, y=25), True), (dict(x=150, y=100), False),
                                     (dict(x=-500, y=100), True), (dict(x=500, y=800), True)]:
            with self.subTest(cursor=cursor):
                with patch.object(popup.backend, 'command', side_effect=[json.dumps(cursor), json.dumps(layers)]):
                    with patch.object(popup, 'close') as close:
                        popup.outside()
                        self.assertEqual(close.called, should_close)

    def test_orphan_recovered_from_layer_pid_without_pid_file(self):
        layers = {'DP-1': {'levels': {'3': [dict(namespace='waybar-control-center', pid=1234)]}}}
        with patch.object(popup.backend, 'command', return_value=json.dumps(layers)):
            with patch.object(popup, '_read_cmdline', return_value=['python3', str(popup.WIFI_PANEL)]):
                self.assertEqual(popup.native_panel_pids('wifi'), {1234})

    def test_stale_pid_never_signals_unrelated_process(self):
        (self.run / 'wifi-panel.pid').write_text('1234')
        with patch.object(popup.backend, 'command', return_value='{}'):
            with patch.object(popup, '_read_cmdline', return_value=['python3', '/tmp/unrelated.py']):
                with patch.object(popup.os, 'kill') as kill:
                    popup.close_native_panel('wifi')
                    kill.assert_not_called()
        self.assertFalse((self.run / 'wifi-panel.pid').exists())

    def test_slow_native_startup_keeps_pid_and_waits_for_mapped_window(self):
        clock = [0.0]
        process = MagicMock(pid=1234)
        process.poll.return_value = None

        def sleep(delay):
            clock[0] += delay
            if clock[0] >= 2.5:
                (self.run / 'wifi-panel.ready').write_text('opening')

        with patch.object(popup.subprocess, 'Popen', return_value=process):
            with patch.object(popup.time, 'monotonic', side_effect=lambda: clock[0]):
                with patch.object(popup.time, 'sleep', side_effect=sleep):
                    popup.open_native_panel('wifi', 100, 50, 380, 560, 0, 'DP-1', 'opening')
        self.assertGreaterEqual(clock[0], 2.5)
        self.assertEqual((self.run / 'wifi-panel.pid').read_text(), '1234')
        process.terminate.assert_not_called()

    def test_failed_native_startup_is_terminated_and_reaped(self):
        process = MagicMock(pid=1234)
        process.poll.return_value = None
        with patch.object(popup.subprocess, 'Popen', return_value=process):
            with patch.object(popup.time, 'monotonic', side_effect=[0, 9]):
                with self.assertRaisesRegex(RuntimeError, 'No se pudo iniciar'):
                    popup.open_native_panel('wifi', 100, 50, 380, 560, 0, 'DP-1', 'opening')
        process.terminate.assert_called_once()
        process.wait.assert_called_once()
        self.assertFalse((self.run / 'wifi-panel.pid').exists())

    def test_media_x_and_escape_hide_before_requesting_cleanup(self):
        media_spec = importlib.util.spec_from_file_location('media_popup', popup.NATIVE_PANELS['media'])
        media = importlib.util.module_from_spec(media_spec)
        media_spec.loader.exec_module(media)
        panel = MagicMock(generation='current')
        with patch.object(media.Gtk, 'main_quit'):
            with patch.object(media.subprocess, 'Popen', side_effect=OSError('unavailable')):
                self.assertTrue(media.MediaWidget.request_close(panel))
        panel.hide.assert_called_once()
        panel.destroy.assert_called_once()
        event = MagicMock(keyval=media.Gdk.KEY_Escape)
        self.assertTrue(media.MediaWidget.on_key(panel, None, event))
        panel.request_close.assert_called_once()

    def test_switching_always_closes_previous_before_opening_next(self):
        visible = set()
        armed = []

        def eww(*args, **_kwargs):
            if args[0] == 'active-windows':
                return '\n'.join(f'{name}: {name}' for name in visible if name.startswith('cc-'))
            if args[0] == 'close':
                visible.difference_update(args[1:])
            if args[0] == 'open':
                self.assertFalse(visible, 'Previous window must close first')
                self.assertTrue(armed[-1], 'Dismissal must be armed before mapping')
                visible.add(args[1])
            return ''

        def close_native(module):
            visible.discard(module)

        def open_native(module, *_args):
            self.assertFalse(visible)
            self.assertTrue(armed[-1])
            visible.add(module)

        with patch.object(popup, 'eww', side_effect=eww), \
             patch.object(popup, 'input_bindings', side_effect=armed.append), \
             patch.object(popup, 'native_panel_pids', side_effect=lambda module: {1234} if module in visible else set()), \
             patch.object(popup, 'close_native_panel', side_effect=close_native), \
             patch.object(popup, 'open_native_panel', side_effect=open_native), \
             patch.object(popup.backend, 'command', return_value='[{"name":"DP-1"}]'), \
             patch.object(popup, 'geometry', return_value=('DP-1', 100, 50, 380, 560)), \
             patch.object(popup, 'screen_index', return_value=0):
            for module in ('wifi', 'volume', 'calendar', 'media', 'power', 'wifi'):
                popup.main([module])
                self.assertEqual(len(visible), 1)
            popup.main(['wifi'])
            self.assertFalse(visible, 'Clicking the current module toggles it closed')
            self.assertFalse((self.run / 'popup.json').exists())


if __name__ == '__main__':
    unittest.main()

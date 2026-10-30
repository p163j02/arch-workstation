"""Popup dismissal and native Hyprland Lua regression checks; no devices changed."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location('popup_dismissal', SCRIPTS / 'popup-manager.py')
popup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(popup)


class PopupTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.run = Path(self.directory.name)
        self.scope = patch.object(popup.backend, 'RUN', self.run)
        self.scope.start()
        self.addCleanup(self.scope.stop)
        (self.run / 'popup.json').write_text(json.dumps({'module': 'volume', 'token': 'current'}))
        self.layers = {'eDP-1': {'levels': {'3': [dict(namespace='waybar-control-center', x=100, y=60, w=380, h=320)],
                                             '2': [dict(namespace='waybar', x=0, y=0, w=1920, h=40)]}}}

    def click(self, x, y, token='current'):
        def command(args):
            return json.dumps(dict(x=x, y=y) if 'cursorpos' in args else self.layers)
        with patch.object(popup.backend, 'command', side_effect=command), patch.object(popup, 'close') as close:
            popup.outside(token)
            return close.call_count

    def test_inside_controls_remain_usable(self):
        self.assertEqual(self.click(200, 100), 0)
        self.assertEqual(self.click(100, 60), 0)

    def test_click_anywhere_outside_including_bar_and_second_monitor(self):
        for point in ((20, 20), (20, 500), (2200, 500), (480, 100), (-500, 200)):
            with self.subTest(point=point):
                self.assertEqual(self.click(*point), 1)

    def test_click_from_previous_popup_cannot_close_new_popup(self):
        self.assertEqual(self.click(20, 500, 'previous'), 0)
        with patch.object(popup, 'close') as close:
            popup.main(['close', 'previous'])
            close.assert_not_called()

    def test_escape_and_x_close(self):
        for args in (['close', 'current'], ['close']):
            with patch.object(popup, 'close') as close:
                popup.main(args)
                close.assert_called_once()

    def test_lua_bindings_pass_all_mouse_buttons_and_preserve_other_shortcuts(self):
        with patch.object(popup.backend, 'command', return_value='ok') as command:
            popup.input_bindings(True)
            enable = command.call_args.args[0]
            self.assertEqual(enable[:2], ['hyprctl', 'eval'])
            popup.input_bindings(False)
            disable = command.call_args.args[0][2]
        # Run the generated Lua with stand-in bind handles, exercising install and cleanup.
        code = '''local created, removed, launched = {}, 0, {}
local cursor = {x=150, y=100}
hl = { dsp = { exec_cmd = function(command) return command end },
       exec_cmd = function(command) launched[#launched+1] = command end,
       get_cursor_pos = function() return cursor end,
       get_layers = function() return {{mapped=true, namespace="waybar-control-center", x=100, y=60, w=380, h=320}} end }
function hl.bind(key, command, flags)
    created[#created + 1] = { key=key, flags=flags, command=command }
    return { unbind = function() removed = removed + 1 end }
end
'''+enable[2]+'''
assert(#created == 9)
assert(created[1].key == "Escape")
for index=2,9 do
    assert(created[index].flags.non_consuming)
    assert(created[index].flags.ignore_mods)
    created[index].command()
    assert(#launched == 0)
end
cursor = {x=2200, y=500}
created[2].command()
assert(#launched == 1 and launched[1]:find("close"))
for index=2,9 do
    assert(type(created[index].command) == "function")
end
'''+disable+'''
assert(removed == 9)
assert(#_G.waybar_control_center_binds == 0)
'''
        subprocess.run(['lua', '-'], input=code, text=True, check=True, capture_output=True)
        self.assertNotIn('hl.unbind(', disable)

    def test_native_bindings_validate_with_installed_hyprland(self):
        with patch.object(popup.backend, 'command', return_value='ok') as command:
            popup.input_bindings(True)
            code = command.call_args.args[0][2]
        path = self.run / 'bindings.lua'
        path.write_text(code)
        result = subprocess.run(['Hyprland', '--verify-config', '-c', str(path)], cwd=self.run,
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('config ok', result.stdout)

    def test_native_pid_guard_never_kills_another_program(self):
        for module in popup.NATIVE_PANELS:
            popup.panel_pid_file(module).write_text('12345')
            with patch.object(popup, '_read_cmdline', return_value=['/usr/bin/other']), patch.object(popup.os, 'kill') as kill:
                popup.close_native_panel(module)
                kill.assert_not_called()
            self.assertFalse(popup.panel_pid_file(module).exists())

    def test_close_covers_both_native_panels_and_eww(self):
        with patch.object(popup, 'input_bindings'), patch.object(popup, 'close_native_panel') as native, patch.object(popup, 'eww') as eww:
            popup.close('cc-volume: cc-volume\n')
            self.assertEqual({call.args[0] for call in native.call_args_list}, {'wifi', 'media'})
            eww.assert_called_with('close', 'cc-volume', check=False)


if __name__ == '__main__':
    unittest.main()

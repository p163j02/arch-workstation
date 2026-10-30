"""Native monitor API and startup integration used by the notebook profile."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


class HyprRuntimeTests(unittest.TestCase):
    def test_launcher_uses_native_lua_and_preserves_shell_arguments_and_workspace(self):
        import json
        import shlex
        path = ROOT / 'hypr/launch-in-workspace.py'
        spec = importlib.util.spec_from_file_location('launcher', path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for workspace in ({'id': 3, 'name': '3'}, {'id': -99, 'name': 'special:magic'}):
            argv = ['launcher', 'kitty', '-e', 'printf', 'hola \"mundo\"; $HOME', 'á']
            with patch.object(module.sys, 'argv', argv), \
                 patch.object(module.subprocess, 'check_output', return_value=json.dumps(workspace)), \
                 patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'ok', '')) as run:
                self.assertEqual(module.main(), 0)
                args = run.call_args.args[0]
            self.assertEqual(args[:2], ['hyprctl', 'eval'])
            expected = "cd " + shlex.quote(os.getcwd()) + " && exec " + shlex.join(argv[1:])
            target = str(workspace['id']) if workspace['id'] >= 0 else workspace['name']
            lua = 'hl = {get_active_workspace = function() return {id=' + str(workspace['id']) + ', name=' + json.dumps(workspace['name']) + '} end, exec_cmd = function(command, rules) assert(command == ' + json.dumps(expected, ensure_ascii=False) + '); assert(rules.workspace == ' + json.dumps(target + ' silent') + ') end }; ' + args[2]
            subprocess.run(['lua', '-'], input=lua, text=True, check=True)

    def test_executable_configs_do_not_keep_legacy_cli_dispatches(self):
        import re
        for directory in ('hypr', 'eww/control-center/scripts', 'waybar'):
            for path in (ROOT / directory).rglob('*'):
                if path.is_file() and path.suffix in ('.py', '.sh', '.lua', '.jsonc') and not path.name.startswith('test_'):
                    text = path.read_text()
                    self.assertIsNone(re.search(r'dispatch (?:workspace|moveworkspacetomonitor|focusmonitor|movecursor|closewindow|exit)\b', text), str(path))

    def test_dispatcher_constructors_are_accepted_by_hyprland_057(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'dispatchers.lua'
            path.write_text("""hl.bind("SUPER + 1", hl.dsp.focus({workspace="1"}))
hl.bind("SUPER + 2", hl.dsp.workspace.move({workspace="2", monitor="eDP-1"}))
hl.bind("SUPER + 3", hl.dsp.cursor.move({x=960, y=540}))
hl.bind("SUPER + 4", hl.dsp.exec_cmd("true", {workspace="1 silent"}))
hl.bind("SUPER + 5", hl.dsp.window.close({window="address:0xabcd"}))
hl.bind("SUPER + 6", hl.dsp.exit())
""")
            result = subprocess.run(['Hyprland', '--verify-config', '-c', str(path)], cwd=directory, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('config ok', result.stdout)

    def test_waybar_upgrade_checks_support_before_replacing_running_bar(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            binaries = home / 'bin'
            binaries.mkdir()
            script = home / 'stub'
            script.write_text("""#!/usr/bin/python3
from pathlib import Path
import os, sys
name = Path(sys.argv[0]).name
with open(os.environ['TEST_LOG'], 'a') as log: log.write(name + ' ' + ' '.join(sys.argv[1:]) + '\\n')
if name == 'pgrep': sys.exit(1)
if name == 'yay':
    if os.environ['YAY_FAIL'] == '1': sys.exit(1)
    with open(os.environ['FAKE_WAYBAR'], 'a') as binary: binary.write('# hl.dsp\\n')
""")
            script.chmod(0o755)
            for name in ('yay', 'hyprctl', 'systemctl', 'pkill', 'pgrep'):
                (binaries / name).symlink_to(script)
            session = home / '.config/hypr/session-start.sh'
            session.parent.mkdir(parents=True)
            session.symlink_to(script)
            waybar = binaries / 'waybar'
            for compatible, fail in ((False, False), (True, False), (False, True)):
                waybar.write_text('#!/bin/sh\n' + ('# hl.dsp\n' if compatible else '') + 'exit 0\n')
                waybar.chmod(0o755)
                log = home / f'{compatible}-{fail}.log'
                env = dict(os.environ, HOME=str(home), XDG_STATE_HOME=str(home / '.state'),
                           HYPRLAND_INSTANCE_SIGNATURE='test', PATH=str(binaries) + ':/usr/bin',
                           TEST_LOG=str(log), FAKE_WAYBAR=str(waybar), YAY_FAIL=str(int(fail)))
                result = subprocess.run(['bash', str(ROOT / 'scripts/upgrade-waybar-lua.sh')],
                                        env=env, capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode == 0, not fail, result.stderr)
                calls = log.read_text()
                self.assertEqual('yay -S waybar-git' in calls, not compatible)
                self.assertEqual('pkill -TERM -x waybar' in calls, not fail)

    def test_monitor_reader_covers_extend_mirror_and_disable(self):
        reader = ROOT / 'hypr/local-settings.lua'
        lua = f'''local captured = {{}}
hl = {{monitor = function(spec) captured[#captured+1] = spec end}}
local settings = dofile("{reader}")
settings.monitor("HDMI-A-1,preferred,0x0,1")
settings.monitor("eDP-1,1920x1080@60,1920x0,1.25")
settings.monitor("HDMI-A-1,preferred,0x0,1,mirror,eDP-1")
settings.monitor("eDP-1,disabled")
settings.monitor("eDP-1,preferred,0x0,1,transform,1,mirror,HDMI-A-1")
assert(captured[1].output == "HDMI-A-1" and captured[1].mode == "preferred")
assert(captured[2].scale == 1.25 and captured[2].position == "1920x0")
assert(captured[3].mirror == "eDP-1")
assert(captured[4].disabled == true)
assert(captured[5].disabled == false)
assert(captured[5].transform == 1 and captured[5].mirror == "HDMI-A-1")
'''
        subprocess.run(['lua', '-'], input=lua, text=True, capture_output=True, check=True)

    def test_monitor_cli_quotes_lua_and_reports_hyprland_failure(self):
        path = ROOT / 'hypr/monitor-set.py'
        spec = importlib.util.spec_from_file_location('monitor_set', path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'ok', '')) as run:
            module.apply('HDMI-A-1,preferred,0x0,1')
            args = run.call_args.args[0]
            self.assertEqual(args[:2], ['hyprctl', 'eval'])
            subprocess.run(['luac', '-p', '-'], input=args[2], text=True, check=True)
        with patch.object(module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'error: invalid monitor', '')):
            with self.assertRaises(RuntimeError):
                module.apply('invalid')

    def test_startup_keeps_panels_working_without_optional_polkit_agent(self):
        with tempfile.TemporaryDirectory() as folder:
            home = Path(folder)
            binaries = home / 'bin'
            binaries.mkdir()
            stub = home / 'stub'
            stub.write_text("""#!/usr/bin/python3
import os, sys
from pathlib import Path
name = Path(sys.argv[0]).name
with open(os.environ['TEST_LOG'], 'a') as log:
    log.write(name + ' ' + ' '.join(sys.argv[1:]) + '\\n')
if name == 'systemctl' and 'cat' in sys.argv:
    sys.exit(int(os.environ['TEST_POLKIT_MISSING']))
if name == 'hyprctl': print('[{}]')
if name == 'pc-settings': sys.exit(1)
""")
            stub.chmod(0o755)
            for name in ('systemctl', 'dbus-update-activation-environment', 'pgrep', 'awww',
                         'wallpaper-studio', 'eww', 'waybar', 'swaync', 'hyprctl', 'pc-settings'):
                (binaries / name).symlink_to(stub)
            for missing in ('0', '1'):
                log = home / f'{missing}.log'
                env = dict(os.environ, HOME=str(home), PATH=str(binaries) + ':/usr/bin',
                           TEST_LOG=str(log), TEST_POLKIT_MISSING=missing)
                result = subprocess.run(['bash', str(ROOT / 'hypr/session-start.sh')],
                                        env=env, capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = log.read_text()
                self.assertIn('waybar-control-dispatch.service', calls)
                self.assertIn('wallpaper-studio --restore', calls)
                self.assertIn('eww open sysmon-widget', calls)
                self.assertIn('eww close sysmon-widget-ext', calls)
                self.assertEqual('systemctl --user start hyprpolkitagent.service' in calls, missing == '0')


if __name__ == '__main__':
    unittest.main()

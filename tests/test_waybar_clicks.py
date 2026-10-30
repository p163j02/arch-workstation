"""Exercise launcher Lua with real argument quoting, without opening user apps."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
LAUNCHER = ROOT / 'desktop/hypr/launch-in-workspace.py'


class LauncherTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='waybar-clicks-')
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.cwd = self.directory / "carpeta con ' comillas"
        self.cwd.mkdir()
        self.env = dict(os.environ, PATH=str(self.directory) + os.pathsep + os.environ['PATH'],
                        CLICK_TEST_DIR=str(self.directory), CLICK_WORKSPACE='1', CLICK_WORKSPACE_NAME='1')
        fake = self.directory / 'hyprctl'
        fake.write_text('''#!/usr/bin/env python3
import os, pathlib, subprocess, sys
assert len(sys.argv) == 3 and sys.argv[1] == 'eval', sys.argv
prelude = ''' + repr('''
hl = {
    get_active_workspace = function()
        if os.getenv('CLICK_NO_WORKSPACE') then return nil end
        return {id = tonumber(os.getenv('CLICK_WORKSPACE')), name = os.getenv('CLICK_WORKSPACE_NAME')}
    end,
    exec_cmd = function(command, rules)
        local root = os.getenv('CLICK_TEST_DIR')
        local file = assert(io.open(root .. '/command', 'wb'))
        file:write(command); file:close()
        file = assert(io.open(root .. '/workspace', 'wb'))
        file:write(rules.workspace); file:close()
    end,
}
''') + '''
result = subprocess.run(['lua', '-'], input=prelude + sys.argv[2], text=True, capture_output=True)
if result.returncode:
    # IPC errors can arrive in stdout with a successful hyprctl exit status.
    print(result.stderr)
else:
    subprocess.run(['sh', '-c', (pathlib.Path(os.environ['CLICK_TEST_DIR']) / 'command').read_text()], check=True)
    print('ok')
''')
        fake.chmod(0o755)
        self.recorder = self.directory / 'record.py'
        self.recorder.write_text('import json, os, pathlib, sys\n'
                                 "pathlib.Path(os.environ['CLICK_TEST_DIR'], 'result.json').write_text("
                                 "json.dumps(dict(args=sys.argv[1:], cwd=os.getcwd())))\n")

    def launch(self, *args):
        return subprocess.run([sys.executable, str(LAUNCHER), *args], env=self.env,
                              cwd=self.cwd, capture_output=True, text=True, timeout=5)

    def test_actual_lua_and_shell_preserve_arguments_and_workspace(self):
        args = ['dos palabras', 'a\'b"c', '$(touch unexpected)', '`touch unexpected`',
                'x; touch unexpected', 'Español α 🐧', 'línea\nsegunda', '\\u0001', '\x01', '']
        result = self.launch(sys.executable, str(self.recorder), *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        captured = json.loads((self.directory / 'result.json').read_text())
        self.assertEqual(captured['args'], args)
        self.assertEqual(captured['cwd'], str(self.cwd))
        self.assertEqual((self.directory / 'workspace').read_text(), '1 silent')
        self.assertFalse((self.cwd / 'unexpected').exists())

    def test_extra_and_named_workspaces(self):
        for workspace, name, target in [('12', '12', '12'), ('-1337', 'special:α', 'special:α')]:
            with self.subTest(workspace=workspace):
                self.env.update(CLICK_WORKSPACE=workspace, CLICK_WORKSPACE_NAME=name)
                result = self.launch(sys.executable, str(self.recorder))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((self.directory / 'workspace').read_text(), target + ' silent')

    def test_compositor_error_is_reported_and_does_not_launch(self):
        self.env['CLICK_NO_WORKSPACE'] = '1'
        result = self.launch(sys.executable, str(self.recorder))
        self.assertEqual(result.returncode, 1)
        self.assertIn('No active workspace', result.stderr)
        self.assertFalse((self.directory / 'result.json').exists())

    def test_missing_command_reports_usage(self):
        result = self.launch()
        self.assertEqual(result.returncode, 2)
        self.assertIn('Usage:', result.stderr)


if __name__ == '__main__':
    unittest.main()

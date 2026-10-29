"""Exercise the listener's startup and JSON/graph publication, not just sensors."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

SPEC = importlib.util.spec_from_file_location('temp_history', Path(__file__).with_name('temp-history.py'))
HISTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HISTORY)

TOKENS = dict(muted='#888888', danger='#ff7189', warning='#ebcb8b', soft='#a55dcc')


class TemperatureHistoryTests(unittest.TestCase):
    def capture_sample(self, readings):
        # Use a private directory and injected sensor readings; no hardware or
        # installed widget files are modified by these regression tests.
        with tempfile.TemporaryDirectory() as runtime:
            output = io.StringIO()
            temporary = contextlib.nullcontext(runtime)
            with patch.object(HISTORY.tempfile, 'TemporaryDirectory', return_value=temporary), \
                 patch.object(HISTORY.subprocess, 'check_output', return_value=json.dumps(readings).encode()) as query, \
                 patch.object(HISTORY, 'chart_colors', return_value=TOKENS), \
                 patch.object(HISTORY.time, 'sleep', side_effect=KeyboardInterrupt), \
                 contextlib.redirect_stdout(output):
                with self.assertRaises(KeyboardInterrupt):
                    HISTORY.main()
            query.assert_called_once_with([sys.executable, str(Path(HISTORY.__file__).with_name('temps.py'))], timeout=4)
            data = json.loads(output.getvalue())
            graphs = {sensor: ET.fromstring(Path(data[f'{sensor}_graph']).read_text())
                      for sensor in ('cpu', 'gpu') if data[f'{sensor}_graph']}
            return data, graphs

    def test_listener_starts_and_publishes_both_temperatures_and_svg_graphs(self):
        data, graphs = self.capture_sample(dict(cpu=51, gpu=63))
        self.assertEqual((data['cpu'], data['gpu']), (51, 63))
        self.assertEqual(set(graphs), {'cpu', 'gpu'})
        for sensor in ('cpu', 'gpu'):
            self.assertEqual(data[f'{sensor}_color'], TOKENS['soft'])
            self.assertEqual(graphs[sensor].attrib['viewBox'], '0 0 112 24')

    def test_unavailable_gpu_does_not_hide_cpu_or_invent_a_zero(self):
        data, graphs = self.capture_sample(dict(cpu=44, gpu=-1))
        self.assertEqual(data['cpu'], 44)
        self.assertEqual(data['gpu'], -1)
        self.assertIn('cpu', graphs)
        self.assertEqual(data['gpu_graph'], '')
        self.assertEqual(data['gpu_color'], TOKENS['muted'])

    def test_listener_recovers_after_sensor_query_failure(self):
        with tempfile.TemporaryDirectory() as runtime:
            output, errors = io.StringIO(), io.StringIO()
            with patch.object(HISTORY.tempfile, 'TemporaryDirectory', return_value=contextlib.nullcontext(runtime)), \
                 patch.object(HISTORY.subprocess, 'check_output', side_effect=[subprocess.TimeoutExpired('temps.py', 4), b'{"cpu":55,"gpu":72}']), \
                 patch.object(HISTORY, 'chart_colors', return_value=TOKENS), \
                 patch.object(HISTORY.time, 'sleep', side_effect=[None, KeyboardInterrupt]), \
                 contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with self.assertRaises(KeyboardInterrupt):
                    HISTORY.main()
            data = json.loads(output.getvalue())
            self.assertEqual((data['cpu'], data['gpu']), (55, 72))
            self.assertEqual(data['gpu_color'], TOKENS['warning'])
            self.assertIn('temp-history:', errors.getvalue())


if __name__ == '__main__':
    unittest.main()

import tempfile
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
from temps import temperatures


class TemperatureTests(unittest.TestCase):
    def test_amd_sensors_and_missing_gpu(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for i, (name, value) in enumerate([('k10temp', 51000), ('amdgpu', 42000)]):
                device = root / f'hwmon{i}'
                device.mkdir()
                (device / 'name').write_text(name)
                (device / 'temp1_input').write_text(str(value))
            with patch('temps.shutil.which', return_value=None):
                self.assertEqual(temperatures(root), {'cpu': 51, 'gpu': 42})
                (root / 'hwmon1/temp1_input').unlink()
                self.assertEqual(temperatures(root)['gpu'], -1)

    def test_nvidia_reading_and_driver_failure(self):
        with tempfile.TemporaryDirectory() as temp, patch('temps.shutil.which', return_value='nvidia-smi'):
            with patch('temps.subprocess.check_output', return_value='63\n'):
                self.assertEqual(temperatures(Path(temp)), {'cpu': -1, 'gpu': 63})
            with patch('temps.subprocess.check_output', side_effect=subprocess.CalledProcessError(1, 'nvidia-smi')):
                self.assertEqual(temperatures(Path(temp))['gpu'], -1)

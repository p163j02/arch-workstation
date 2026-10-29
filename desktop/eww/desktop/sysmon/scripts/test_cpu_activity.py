#!/usr/bin/env python3
"""Offline tests: no device changes, no Eww daemon required."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

SPEC = importlib.util.spec_from_file_location('cpu_activity', Path(__file__).with_name('cpu-activity.py'))
CPU = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CPU)


class CPUActivityTests(unittest.TestCase):
    def test_guest_time_is_not_counted_twice(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'stat'
            path.write_text('cpu 100 10 20 300 40 2 3 5 99 8\ncpu0 0 0 0 0\n')
            self.assertEqual(CPU.counters(path), (480, 340))

    def test_usage_and_bounds(self):
        self.assertEqual(CPU.utilization((100, 50), (200, 100)), 50)
        self.assertEqual(CPU.utilization((100, 50), (100, 50)), 0)
        self.assertEqual(CPU.utilization((100, 50), (90, 50)), 0)
        self.assertEqual(CPU.utilization((100, 50), (200, 50)), 100)
        self.assertEqual(CPU.utilization((100, 50), (200, 200)), 0)

    def test_svg_and_scale(self):
        for samples, expected in [([3], 10), ([1, 3, 21], 30), ([0, 100], 100)]:
            svg, ceiling = CPU.chart(samples)
            self.assertEqual(ceiling, expected)
            root = ET.fromstring(svg)
            self.assertEqual(root.attrib['viewBox'], '0 0 304 52')

    def test_first_sample_is_not_a_fabricated_curve(self):
        svg, _ = CPU.chart([3])
        root = ET.fromstring(svg)
        ns = {'svg': 'http://www.w3.org/2000/svg'}
        self.assertEqual(len(root.findall('svg:circle', ns)), 1)
        self.assertEqual(len(root.findall('svg:polyline', ns)), 0)


if __name__ == '__main__':
    unittest.main()

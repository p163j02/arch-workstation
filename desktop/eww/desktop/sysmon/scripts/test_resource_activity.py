#!/usr/bin/env python3
"""Offline checks for RAM bars and real block-device throughput."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

SPEC = importlib.util.spec_from_file_location('resource_activity', Path(__file__).with_name('resource-activity.py'))
RESOURCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RESOURCE)
NS = {'svg': 'http://www.w3.org/2000/svg'}


class ResourceActivityTests(unittest.TestCase):
    def test_sector_units_are_512_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            device = Path(directory)
            (device / 'stat').write_text('10 0 2048 0 20 0 4096 0 0 0 0')
            self.assertEqual(RESOURCE.disk_counters([device]), (1024**2, 2 * 1024**2))

    def test_real_elapsed_time_and_read_write_independent(self):
        self.assertEqual(RESOURCE.throughput((100, 200), (1124, 4296), 2), (512, 2048))

    def test_startup_and_reset_are_not_fabricated_zeros(self):
        self.assertIsNone(RESOURCE.throughput(None, (100, 200), 2))
        self.assertIsNone(RESOURCE.throughput((100, 200), (0, 300), 2))
        self.assertIsNone(RESOURCE.throughput((100, 200), (100, 200), 0))
        self.assertEqual(RESOURCE.throughput((100, 200), (100, 200), 2), (0, 0))

    def test_partition_resolves_to_parent_disk(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            disk = base / 'devices' / 'nvme0n1'
            partition = disk / 'nvme0n1p3'
            partition.mkdir(parents=True)
            (partition / 'partition').touch()
            sysfs = base / 'dev'
            sysfs.mkdir()
            (sysfs / '259:3').symlink_to(partition)
            with patch.object(RESOURCE.os, 'major', return_value=259), patch.object(RESOURCE.os, 'minor', return_value=3):
                self.assertEqual(RESOURCE.root_devices(sysfs, base), [disk])

    def test_stacked_devices_do_not_double_count(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            disk = base / 'sda'
            disk.mkdir()
            dm = base / 'dm-0'
            slaves = dm / 'slaves'
            slaves.mkdir(parents=True)
            (slaves / 'sda').symlink_to(disk)
            sysfs = base / 'dev'
            sysfs.mkdir()
            (sysfs / '253:0').symlink_to(dm)
            with patch.object(RESOURCE.os, 'major', return_value=253), patch.object(RESOURCE.os, 'minor', return_value=0):
                self.assertEqual(RESOURCE.root_devices(sysfs, base), [disk])

    def test_first_chart_sample_is_only_a_point(self):
        svg, _ = RESOURCE.disk_chart([1024])
        root = ET.fromstring(svg)
        self.assertEqual(len(root.findall('svg:circle', NS)), 1)
        self.assertIsNone(root.find('svg:polyline', NS))

    def test_throughput_scale_not_capacity_percent(self):
        svg, ceiling = RESOURCE.disk_chart([0, 1024**2, 2 * 1024**2])
        root = ET.fromstring(svg)
        self.assertEqual(ceiling, 2 * 1024**2)
        self.assertEqual(root.attrib['viewBox'], '0 0 304 48')
        self.assertEqual(root.find('svg:polyline', NS).attrib['points'], '0.0,46.0 152.0,26.0 304.0,6.0')

    def test_zero_activity_and_rate_units(self):
        _, ceiling = RESOURCE.disk_chart([0, 0])
        self.assertEqual(ceiling, 1024**2)
        self.assertEqual(RESOURCE.format_rate(None), '—')
        self.assertEqual(RESOURCE.format_rate(0), '0 B/s')
        self.assertEqual(RESOURCE.format_rate(2 * 1024**2), '2.0 MiB/s')

    def test_ram_payload_has_no_graph_or_swap(self):
        with patch.object(RESOURCE.subprocess, 'check_output', return_value=b'{"ram_pct":32,"ram_used_gb":"4.3","ram_total_gb":"13.5","swap_pct":0}'):
            data = RESOURCE.occupancy('ram')
        self.assertEqual(data['ram_pct'], 32)
        self.assertNotIn('graph', data)
        self.assertNotIn('swap_pct', data)


if __name__ == '__main__':
    unittest.main()

#!/usr/bin/env python3
"""Non-destructive tests: no network changes, real portals, or browser launches."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

spec = importlib.util.spec_from_file_location('wifi_portal', Path(__file__).resolve().parents[1] / 'scripts/wifi-portal.py')
portal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(portal)


class PortalTests(unittest.TestCase):
    def setUp(self):
        self.device = dict(DeviceType=2, State=100, ActiveConnection='/active/1', Ip4Connectivity=2, Ip6Connectivity=3)
        self.connection = dict(Default=True, Default6=False)

    def eligible(self, connectivity=2, primary='/active/1'):
        return portal.eligible(self.device, self.connection, primary, connectivity)

    def test_portal_detected(self):
        self.assertTrue(self.eligible())

    def test_offline_limited_unknown_do_not_launch(self):
        for status in (0, 1, 3):
            self.device['Ip4Connectivity'] = status
            self.assertFalse(self.eligible(status))

    def test_full_connectivity_prevents_launch(self):
        self.assertFalse(self.eligible(4))
        self.device['Ip6Connectivity'] = 4
        self.assertFalse(self.eligible())

    def test_non_wifi_or_inactive_do_not_launch(self):
        self.device['DeviceType'] = 1
        self.assertFalse(self.eligible())
        self.device['DeviceType'], self.device['State'] = 2, 40
        self.assertFalse(self.eligible())

    def test_secondary_route_or_vpn_do_not_launch(self):
        self.assertFalse(self.eligible(primary='/vpn/1'))
        self.connection['Default'] = False
        self.assertFalse(self.eligible())

    def test_one_launch_despite_repeated_events_and_restart(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'state.json'
            once, launch = portal.OncePerConnection(path), Mock()
            once.update({'wifi:1'}, {'wifi:1'}, launch)
            once.update({'wifi:1'}, {'wifi:1'}, launch)
            once.update({'wifi:1'}, set(), launch)  # Login succeeds.
            once.update({'wifi:1'}, {'wifi:1'}, launch)  # Session expires: no tab flood.
            portal.OncePerConnection(path).update({'wifi:1'}, {'wifi:1'}, launch)
            launch.assert_called_once_with()
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)

    def test_reconnect_opens_again(self):
        with tempfile.TemporaryDirectory() as temp:
            once, launch = portal.OncePerConnection(Path(temp) / 'state.json'), Mock()
            once.update({'wifi:1'}, {'wifi:1'}, launch)
            once.update(set(), set(), launch)
            once.update({'wifi:2'}, {'wifi:2'}, launch)
            self.assertEqual(launch.call_count, 2)

    def test_new_connection_without_observed_disconnect(self):
        with tempfile.TemporaryDirectory() as temp:
            once, launch = portal.OncePerConnection(Path(temp) / 'state.json'), Mock()
            once.update({'wifi:1'}, {'wifi:1'}, launch)
            once.update({'wifi:2'}, {'wifi:2'}, launch)
            self.assertEqual(launch.call_count, 2)
            self.assertEqual(once.opened, {'wifi:2'})

    def test_failed_spawn_can_retry(self):
        with tempfile.TemporaryDirectory() as temp:
            once = portal.OncePerConnection(Path(temp) / 'state.json')
            with self.assertRaises(OSError):
                once.update({'wifi:1'}, {'wifi:1'}, Mock(side_effect=OSError('test')))
            self.assertEqual(once.opened, set())
            launch = Mock()
            once.update({'wifi:1'}, {'wifi:1'}, launch)
            launch.assert_called_once_with()

    def test_fixed_http_url(self):
        self.assertEqual(portal.PORTAL_URL, 'http://neverssl.com/')


if __name__ == '__main__':
    unittest.main()

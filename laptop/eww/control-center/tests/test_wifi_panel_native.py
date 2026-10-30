#!/usr/bin/env python3
"""Non-destructive tests for native Wi-Fi popup helpers."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / 'scripts'
sys.path.insert(0, str(SCRIPTS))

wifi_spec = importlib.util.spec_from_file_location('wifi_panel', SCRIPTS / 'wifi-panel.py')
wifi_panel = importlib.util.module_from_spec(wifi_spec)
wifi_spec.loader.exec_module(wifi_panel)

popup_spec = importlib.util.spec_from_file_location('popup_manager', SCRIPTS / 'popup-manager.py')
popup_manager = importlib.util.module_from_spec(popup_spec)
popup_spec.loader.exec_module(popup_manager)


class NativeWifiHelpers(unittest.TestCase):
    def test_wallpaper_theme_refresh_preserves_last_good_css(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory, patch.object(wifi_panel, 'ROOT', Path(directory)):
            panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
            panel.running = True
            panel.theme_provider = wifi_panel.Gtk.CssProvider()
            panel.theme_signature = None
            path = Path(directory) / 'wallpaper-studio.scss'
            path.write_text('.cc-panel {color: #e865a3;}')
            self.assertTrue(panel.refresh_theme())
            previous = panel.theme_signature
            before = panel.theme_provider.to_string()
            path.write_text('.cc-panel { color: invalid-value; }')
            self.assertTrue(panel.refresh_theme())
            self.assertEqual(panel.theme_signature, previous)
            # Provider changes are staged, so failed parsing preserves the theme.
            self.assertEqual(panel.theme_provider.to_string(), before)
            path.write_text('.cc-panel {color: #2c80e0;}')
            self.assertTrue(panel.refresh_theme())
            self.assertNotEqual(panel.theme_provider.to_string(), before)
            panel.running = False
            self.assertFalse(panel.refresh_theme())

    def test_saved_personal_network_prompts_without_activation(self):
        panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
        panel.local_busy = False
        panel.initializing = False
        panel.state = {'busy': False, 'enabled': True}
        panel.saved_ssids = {'Personal'}
        panel.ap_meta = {'AA:BB:CC:DD:EE:FF': {'security': 'psk'}}
        panel.show_password_prompt = MagicMock()
        panel.run_task = MagicMock()
        network = {'id': 'AA:BB:CC:DD:EE:FF', 'name': 'Personal', 'secure': True}
        panel.on_network(None, network)
        panel.show_password_prompt.assert_called_once()
        panel.run_task.assert_not_called()

    def test_prompt_enables_children_before_show_all(self):
        panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
        panel.saved_ssids = set()
        for name in ('password_title', 'password_hint', 'password_entry', 'show_btn', 'password_box', 'saved_btn'):
            setattr(panel, name, MagicMock())
        panel.scroll_to_password = lambda: False
        panel.show_password_prompt({'name': 'Personal'}, 'Contraseña')
        panel.password_box.set_no_show_all.assert_called_once_with(False)
        panel.password_box.show_all.assert_called_once()
        panel.password_entry.grab_focus.assert_called_once()

    def test_authentication_failure_message(self):
        def fake_get(_bus, _path, _iface, prop):
            if prop == 'State':
                return int(wifi_panel.NM.ActiveConnectionState.DEACTIVATED)
            return (120, int(wifi_panel.NM.DeviceStateReason.NO_SECRETS))
        with self.assertRaisesRegex(wifi_panel.ConnectionFailure, 'contraseña puede ser incorrecta'):
            wifi_panel.wait_for_activation(None, '/active', interval=0,
                prop_getter=fake_get, device_path='/device')

    def test_password_failure_restores_prompt_without_raw_error(self):
        panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
        panel.show_password_prompt = MagicMock()
        network = {'name': 'Personal'}
        panel.handle_password_error(network, RuntimeError('raw private dbus error'))
        panel.show_password_prompt.assert_called_once()
        self.assertEqual(panel.local_error, wifi_panel.generic_connect_error())
        self.assertNotIn('private', panel.local_error)

    def test_spinner_animation_stops_when_idle(self):
        panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
        panel.running = True
        panel.local_busy = True
        panel.initializing = False
        panel.state = {}
        panel.spinner_frame = 0
        panel.loading_icon = MagicMock()
        self.assertTrue(panel.animate_spinner())
        panel.loading_icon.set_text.assert_called_once()
        panel.local_busy = False
        panel.loading_icon.reset_mock()
        panel.animate_spinner()
        panel.loading_icon.set_text.assert_not_called()
        panel.running = False
        self.assertFalse(panel.animate_spinner())

    def test_security_classification(self):
        sec = getattr(wifi_panel.NM, '80211ApSecurityFlags')
        self.assertEqual(wifi_panel.classify_security(False, 0, 0), 'open')
        self.assertEqual(wifi_panel.classify_security(True, 0, 0), 'wep')
        self.assertEqual(wifi_panel.classify_security(True, int(sec.KEY_MGMT_PSK), 0), 'psk')
        self.assertEqual(wifi_panel.classify_security(True, 0, int(sec.KEY_MGMT_802_1X)), 'enterprise')

    def test_psk_key_mgmt_selection(self):
        sec = getattr(wifi_panel.NM, '80211ApSecurityFlags')
        self.assertEqual(wifi_panel.choose_psk_key_mgmts(0, int(sec.KEY_MGMT_PSK)), ['wpa-psk'])
        self.assertEqual(wifi_panel.choose_psk_key_mgmts(0, int(sec.KEY_MGMT_SAE)), ['sae', 'wpa-psk'])

    def test_psk_validation(self):
        self.assertTrue(wifi_panel.valid_psk('abcdefgh'))
        self.assertTrue(wifi_panel.valid_psk('a' * 63))
        self.assertTrue(wifi_panel.valid_psk('A' * 64))
        self.assertFalse(wifi_panel.valid_psk('short'))

    def test_psk_settings_shape(self):
        settings = wifi_panel.build_psk_settings('TestNet', 'secret-pass', 'wpa-psk')
        self.assertIn('802-11-wireless-security', settings)
        self.assertEqual(settings['802-11-wireless-security']['key-mgmt'].unpack(), 'wpa-psk')
        self.assertEqual(settings['802-11-wireless-security']['psk'].unpack(), 'secret-pass')

    def test_open_settings_shape(self):
        settings = wifi_panel.build_open_settings('Cafe')
        self.assertIn('802-11-wireless', settings)
        self.assertNotIn('802-11-wireless-security', settings)
        self.assertEqual(bytes(settings['802-11-wireless']['ssid'].unpack()), b'Cafe')

    def test_profile_iface_scope(self):
        self.assertTrue(wifi_panel.profile_iface_matches('', 'wlan0'))
        self.assertTrue(wifi_panel.profile_iface_matches('wlan0', 'wlan0'))
        self.assertFalse(wifi_panel.profile_iface_matches('wlan1', 'wlan0'))

    def test_wait_for_activation_success(self):
        state = getattr(wifi_panel.NM, 'ActiveConnectionState')
        sequence = iter([int(state.ACTIVATING), int(state.ACTIVATED)])

        def fake_get(_bus, _path, _iface, _name):
            return next(sequence)

        wifi_panel.wait_for_activation(None, '/active', timeout=1, interval=0, prop_getter=fake_get)

    def test_wait_for_activation_failure(self):
        state = getattr(wifi_panel.NM, 'ActiveConnectionState')
        sequence = iter([int(state.ACTIVATING), int(state.DEACTIVATED)])

        def fake_get(_bus, _path, _iface, _name):
            return next(sequence)

        with self.assertRaises(RuntimeError):
            wifi_panel.wait_for_activation(None, '/active', timeout=1, interval=0, prop_getter=fake_get)

    def test_initialization_waits_for_both_data_sources(self):
        panel = wifi_panel.WifiPanel.__new__(wifi_panel.WifiPanel)
        panel.initializing = True
        panel.client = None
        panel.snapshot_ready = True
        panel.refresh_saved_profiles = MagicMock()
        panel.refresh_ap_meta = MagicMock()
        panel.update_initialization()
        self.assertTrue(panel.initializing)
        panel.refresh_saved_profiles.assert_not_called()
        panel.client = MagicMock()
        panel.update_initialization()
        self.assertFalse(panel.initializing)
        panel.refresh_saved_profiles.assert_called_once()
        panel.refresh_ap_meta.assert_called_once()

    def test_popup_uses_native_dispatcher_with_python_fallback(self):
        shebang = (SCRIPTS / 'popup-manager.py').read_text().splitlines()[0]
        self.assertIn(shebang, ('#!/usr/bin/env python3', '#!' + str(SCRIPTS / 'fast-client')))
        self.assertTrue((SCRIPTS / 'fast-client.c').is_file())
        if shebang != '#!/usr/bin/env python3':
            self.assertTrue((SCRIPTS / 'fast-client').is_file())

    def test_popup_cmdline_guard(self):
        self.assertTrue(popup_manager._is_panel_cmdline(['/usr/bin/python3', str(popup_manager.NATIVE_PANELS['wifi'])], 'wifi'))
        self.assertFalse(popup_manager._is_panel_cmdline(['/usr/bin/python3', '/tmp/wifi-panel.py'], 'wifi'))
        self.assertFalse(popup_manager._is_panel_cmdline(['/usr/bin/python3', '/tmp/other.py'], 'wifi'))


if __name__ == '__main__':
    unittest.main(verbosity=2)

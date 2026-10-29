#!/usr/bin/env python3
"""Non-destructive adapter/protocol tests; fake events are confined to tests."""
import datetime as dt
import json
import unittest
from unittest.mock import patch
from pathlib import Path
import sys
import tempfile

# Resolve the canonical location even when invoked through scripts/test_adapters.py.
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import backend
import airpods
import calendar_state

class Adapters(unittest.TestCase):
    def test_volume_level_without_sink_enumeration(self):
        with patch.object(backend, 'command', return_value='Volume: 0.42 [MUTED]') as command:
            self.assertEqual(backend.volume_level(), (42, True))
            command.assert_called_once_with(['wpctl', 'get-volume', '@DEFAULT_AUDIO_SINK@'])

    def test_volume_slider_drains_latest_target(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(backend, 'RUN', Path(temp)), \
             patch.object(backend, 'volume_level', return_value=(10, False)), \
             patch.object(backend.time, 'sleep'):
            targets=[]
            def write(args):
                targets.append(args[-1])
                if len(targets)==1:
                    backend.write_pending(Path(temp)/'volume-pending', 30)
                elif len(targets)==2:
                    backend.write_pending(Path(temp)/'volume-pending', 40)
                return ''
            with patch.object(backend, 'command', side_effect=write):
                backend.action('volume', 'set', '20')
            self.assertEqual(targets, ['20.0%', '30%', '40%'])

    def test_confirmed_ddc_write_keeps_cache(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(backend, 'RUN', Path(temp)), \
             patch.object(backend, 'backlight', return_value=(Path('/fake/backlight'), [])), \
             patch.object(backend, 'external_brightness', return_value=dict(available=True,bus='4',maximum=100)), \
             patch.object(backend, 'command', return_value=''):
            backend.set_displays(42)
            saved=json.loads((Path(temp)/'ddc-state.json').read_text())
            self.assertEqual(saved['state']['percent'], 42)
            self.assertEqual(saved['state']['bus'], '4')

    def test_refresh_notification_atomic(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(backend, 'RUN', Path(temp)):
            backend.notify_refresh('volume')
            self.assertTrue((Path(temp)/'volume-refresh').read_text().isdigit())
            self.assertEqual(len(list(Path(temp).iterdir())), 1)

    def test_battery_duration(self):
        self.assertEqual(backend.battery_duration(401),'6 h 41 min')
        self.assertEqual(backend.battery_duration(60),'1 h 0 min')
        self.assertEqual(backend.battery_duration(45),'45 min')
    def test_nmcli_escaping(self):
        self.assertEqual(backend.fields(r'*:AA\:BB\:CC\:DD\:EE\:FF:evil\:name\\test:70:WPA2'),['*','AA:BB:CC:DD:EE:FF','evil:name\\test','70','WPA2'])
    def test_numeric_limits(self):
        for v in ('-1','101','nan','inf','$(touch /tmp/nope)'):
            with self.assertRaises(ValueError): backend.numeric(v)
    def test_unknown_battery(self):
        s=airpods.State()
        with patch.object(backend,'bt_state',return_value={'devices':[]}):
            d=s.status()
        self.assertEqual([d['left'],d['right'],d['case']],['—']*3)
    def test_snapshot_battery_is_not_fresh(self):
        s=airpods.State()
        s.sock=True
        mac=backend.config()['airpods_address']
        s.event({'AACPEvent':[mac,{'BatteryInfo':[{'component':4,'level':62,'status':2},{'component':8,'level':101,'status':2}]}]})
        with patch.object(backend,'bt_state',return_value={'devices':[{'id':mac,'name':'Test','connected':True,'battery':'—'}]}):
            d=s.status()
        self.assertEqual(d['left'],'62% · anterior')
        self.assertEqual(d['case'],'—')
        self.assertFalse(d['fresh'])
    def test_automatic_airpods_model(self):
        s=airpods.State()
        mac=backend.config()['airpods_address']
        device={'id':mac,'name':'AirPods','connected':False,'battery':'—','modalias':'bluetooth:v004Cp200FdD415'}
        with patch.object(backend,'bt_state',return_value={'devices':[device]}):
            d=s.status()
        self.assertEqual(d['model'],'AirPods · 2ª generación')
        self.assertEqual(d['note'],'')
    def test_power_confirmation_expired(self):
        with patch.object(backend.Path,'read_text',return_value=json.dumps({'action':'shutdown','time':0})):
            self.assertEqual(backend.power_state()['pending'],'')
    def test_calendar_monday_and_leap(self):
        s={'year':2024,'month':2,'selected':'2024-02-29','today':dt.date.today().isoformat()}
        with patch.object(calendar_state,'get',return_value=s):
            d=calendar_state.status()
        self.assertEqual(dt.date.fromisoformat(d['weeks'][0][0]['id']).weekday(),0)
        self.assertTrue(any(cell['selected'] and cell['day']==29 for w in d['weeks'] for cell in w))
        self.assertIn('febrero',d['selected'])

if __name__=='__main__': unittest.main(verbosity=2)

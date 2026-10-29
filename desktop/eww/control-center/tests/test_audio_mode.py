"""Non-destructive mono routing tests."""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import backend


class AudioModeTests(unittest.TestCase):
    def test_invalid_mode_never_runs_command(self):
        with patch.object(backend, 'command') as command:
            with self.assertRaises(ValueError):
                backend.set_audio_mode('invalid')
            command.assert_not_called()

    def test_stereo_restores_mono_volume_not_stale_wpctl(self):
        sinks = [dict(name='physical'), dict(name=backend.MONO_SINK,
            owner_module=42, mute=True, volume={'mono': {'value': 32768}})]
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(backend, 'RUN', Path(directory)), \
             patch.object(backend, 'audio_sinks', return_value=sinks), \
             patch.object(backend, 'move_audio_streams') as move, \
             patch.object(backend, 'command', return_value=backend.MONO_SINK) as command:
            (Path(directory) / 'mono-master').write_text('physical')
            backend.set_audio_mode('stereo')
            command.assert_any_call(['pactl', 'set-sink-volume', 'physical', '50%'])
            command.assert_any_call(['pactl', 'set-sink-mute', 'physical', '1'])
            command.assert_any_call(['pactl', 'unload-module', '42'])
            move.assert_called_once_with('physical', 42)

    def test_mono_volume_set_after_physical_volume(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(backend, 'RUN', Path(directory)), \
             patch.object(backend, 'audio_sinks', return_value=[dict(name='physical')]), \
             patch.object(backend, 'volume_level', return_value=(25, False)), \
             patch.object(backend, 'move_audio_streams'), \
             patch.object(backend, 'command') as command:
            command.side_effect = lambda args: 'physical' if args[1]=='get-default-sink' else '42' if args[1]=='load-module' else ''
            backend.set_audio_mode('mono')
            calls = [call.args[0] for call in command.call_args_list]
            self.assertLess(calls.index(['pactl', 'set-sink-volume', 'physical', '100%']),
                calls.index(['pactl', 'set-sink-volume', backend.MONO_SINK, '25%']))


if __name__ == '__main__':
    unittest.main()

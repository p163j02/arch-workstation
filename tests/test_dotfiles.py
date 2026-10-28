import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import hardware
import check
from portable import transform


class PortableTests(unittest.TestCase):
    def test_secret_fields_are_rejected_at_any_depth(self):
        check.reject_credentials({'options': {'apiKey': '{env:LOCAL_API_KEY}'}}, 'test.json')
        with self.assertRaisesRegex(ValueError, 'Credencial literal'):
            check.reject_credentials({'providers': [{'access_token': 'local-' + 'only'}]}, 'test.json')

    def test_private_key_is_rejected_without_printing_it(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            content = '-----BEGIN ' + 'PRIVATE KEY-----\nnot-a-real-key\n'
            (root / 'unexpected.txt').write_text(content)
            with self.assertRaisesRegex(ValueError, 'Posible secreto') as error:
                check.check_secrets(root)
            self.assertNotIn('not-a-real-key', str(error.exception))

    def test_forced_local_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'settings.local.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'no permitido'):
                check.check_secrets(root)

    def test_hybrid_gpu_and_cpu_packages(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cpu = root / 'cpuinfo'
            cpu.write_text('vendor_id: GenuineIntel\n')
            pci = root / 'pci'
            for name, vendor, device_class in [('igpu', '0x8086', '0x030000'),
                                               ('dgpu', '0x10de', '0x030200'),
                                               ('other', '0x1002', '0x040000')]:
                device = pci / name
                device.mkdir(parents=True)
                (device / 'class').write_text(device_class)
                (device / 'vendor').write_text(vendor)
            names = hardware.packages(cpu, pci)
            self.assertIn('intel-ucode', names)
            self.assertIn('vulkan-intel', names)
            self.assertIn('nvidia-open-dkms', names)
            self.assertNotIn('vulkan-radeon', names)
            cpu.write_text('vendor_id: AuthenticAMD\n')
            self.assertIn('amd-ucode', hardware.packages(cpu, pci))

    def test_template_roundtrip_and_local_shebang(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            p = root / 'config'
            p.write_text('path=@HOME@/.config/example\n')
            transform(root, temp, render=True)
            self.assertEqual(p.read_text(), f'path={temp}/.config/example\n')
            transform(root, temp)
            self.assertEqual(p.read_text(), 'path=@HOME@/.config/example\n')
            p.write_text(f'#!{temp}/fast-client\nprint(1)\n')
            transform(root, temp)
            self.assertEqual(p.read_text(), '#!/usr/bin/env python3\nprint(1)\n')

    def test_jsonc_preserves_urls_and_string_punctuation(self):
        self.assertEqual(check.jsonc('{"url":"https://example.org/a,}",/*x*/"a":[1,],}'),
                         {'url': 'https://example.org/a,}', 'a': [1]})

    def test_missing_aur_package_fails(self):
        with patch.object(check.subprocess, 'check_output', return_value='\n'.join(
            check.package_names(ROOT) + ['amd-ucode', 'intel-ucode', 'nvidia-open-dkms',
            'nvidia-utils', 'lib32-nvidia-utils', 'nvidia-settings', 'mesa', 'lib32-mesa',
            'vulkan-radeon', 'lib32-vulkan-radeon', 'vulkan-intel', 'lib32-vulkan-intel'])):
            import io
            with patch.object(check.urllib.request, 'urlopen', return_value=io.BytesIO(
                json.dumps({'type': 'multiinfo', 'results': []}).encode())):
                with self.assertRaisesRegex(ValueError, 'AUR ausentes'):
                    check.check_packages(ROOT)


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='dotfiles-test-')
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name) / 'home'
        self.home.mkdir()
        self.env = dict(os.environ, HOME=str(self.home), XDG_CONFIG_HOME=str(self.home / '.config'),
                        XDG_DATA_HOME=str(self.home / '.local/share'), XDG_STATE_HOME=str(self.home / '.local/state'),
                        XDG_RUNTIME_DIR=str(self.home / 'run'), PYTHONDONTWRITEBYTECODE='1')
        Path(self.env['XDG_RUNTIME_DIR']).mkdir()

    def install(self, profile, repo=ROOT):
        result = subprocess.run(['bash', str(repo / profile / 'install.sh'), '--config-only'],
                                cwd=self.temp.name, env=self.env, input='si\n',
                                capture_output=True, text=True, timeout=45)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return self.home / '.config'

    def test_commit_hook_checks_staged_content(self):
        repo = Path(self.temp.name) / 'hook-repo'
        (repo / 'scripts').mkdir(parents=True)
        for name in ('check.py', 'portable.py'):
            shutil.copy2(ROOT / 'scripts' / name, repo / 'scripts' / name)
        subprocess.run(['git', 'init', '-q', str(repo)], check=True)
        private = repo / 'settings.json'
        private.write_text(json.dumps({'password': 'local-' + 'only'}))
        subprocess.run(['git', '-C', str(repo), 'add', '.'], check=True)
        private.write_text('{}')
        result = subprocess.run(['bash', str(ROOT / '.githooks/pre-commit')],
                                cwd=repo, env=self.env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Credencial literal', result.stdout + result.stderr)
        self.assertNotIn('local-' + 'only', result.stdout + result.stderr)

    def test_both_profiles_from_an_empty_home(self):
        for profile in ('laptop', 'desktop'):
            with self.subTest(profile=profile):
                config = self.install(profile)
                self.assertEqual((config / 'dotfiles/profile').read_text().strip(), profile)
                for path in ['hypr/hyprlock.conf', 'hypr/nightlight.sh', 'hypr/launch-in-workspace.py',
                             'waybar/scripts/media-status.sh', 'eww/control-center/scripts/fast-client',
                             'systemd/user/waybar-control-dispatch.service']:
                    self.assertTrue((config / path).is_file(), path)
                self.assertTrue((config / 'eww/scripts').is_dir())
                self.assertNotIn('@HOME@', (config / 'rofi/config.rasi').read_text())
                shebang = (config / 'eww/control-center/scripts/popup-manager.py').read_text().splitlines()[0]
                self.assertEqual(shebang, '#!' + str(config / 'eww/control-center/scripts/fast-client'))
                self.assertEqual((config / 'hypr/battery-monitor.sh').exists(), profile == 'laptop')
                self.assertFalse((config / 'opencode/service.json').exists())

    def test_reinstall_backs_up_and_preserves_private_settings(self):
        config = self.install('laptop')
        (config / 'kitty/kitty.conf').write_text('old-settings\n')
        (config / 'hypr/local.conf').write_text('kb_layout = us\n')
        (config / 'opencode/service.json').write_text('{"password":"local-only"}\n')
        (config / 'eww/control-center/settings.local.json').write_text('{"brightness_min":9}\n')
        self.install('laptop')
        self.assertEqual((config / 'hypr/local.conf').read_text(), 'kb_layout = us\n')
        self.assertIn('local-only', (config / 'opencode/service.json').read_text())
        self.assertIn('9', (config / 'eww/control-center/settings.local.json').read_text())
        backups = self.home / '.local/state/dotfiles/backups'
        self.assertTrue(any(p.read_text() == 'old-settings\n' for p in backups.glob('*/config/kitty/kitty.conf')))

    def test_update_keeps_profiles_separate_and_omits_secrets(self):
        repo = Path(self.temp.name) / 'repo'
        shutil.copytree(ROOT, repo, symlinks=True,
                        ignore=shutil.ignore_patterns('.git', 'node_modules', '__pycache__', 'graphify-out'))
        config = self.install('desktop', repo)
        marker = repo / 'laptop/hypr/marker'
        marker.write_text('preserve notebook\n')
        (config / 'opencode/service.json').write_text('{"password":"local-only"}\n')
        command = ['bash', str(repo / 'update.sh')]
        wrong = subprocess.run(command + ['laptop'], env=self.env, capture_output=True, text=True)
        self.assertNotEqual(wrong.returncode, 0)
        (config / 'kitty/kitty.conf').write_text('font_size 11\n')
        result = subprocess.run(command + ['desktop'], env=self.env, capture_output=True, text=True, timeout=45)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('Actualizado el perfil desktop', result.stdout, result.stdout + result.stderr)
        self.assertEqual(marker.read_text(), 'preserve notebook\n')
        self.assertEqual((repo / 'shared/kitty/kitty.conf').read_text(), 'font_size 11\n')
        self.assertFalse((repo / 'shared/opencode/service.json').exists())
        self.assertEqual((repo / 'shared/eww/control-center/scripts/backend.py').read_text().splitlines()[0],
                         '#!/usr/bin/env python3')

    def test_media_metadata_is_valid_json(self):
        binaries = self.home / 'bin'
        binaries.mkdir()
        player = binaries / 'playerctl'
        player.write_text('#!/usr/bin/env python3\nimport sys\n'
                          'print("Playing" if "status" in sys.argv else \'Song "quoted" \\\\ Live\\nPart 2\')\n')
        player.chmod(0o755)
        env = dict(self.env, PATH=str(binaries) + ':' + os.environ['PATH'])
        output = subprocess.check_output(['bash', str(ROOT / 'shared/waybar/scripts/media-status.sh')],
                                         env=env, text=True)
        self.assertIn('\\ Live', json.loads(output)['tooltip'])

    def test_package_failure_never_copies_configs_or_reports_success(self):
        binaries = self.home / 'bin'
        binaries.mkdir()
        stubs = {
            'pacman-conf': '#!/bin/bash\nprintf "core\\nextra\\nmultilib\\n"\n',
            'pacman': '#!/bin/bash\nprintf "linux\\n"\n',
            'sudo': '#!/bin/bash\nexit 55\n',
        }
        for name, text in stubs.items():
            path = binaries / name
            path.write_text(text)
            path.chmod(0o755)
        result = subprocess.run(['bash', str(ROOT / 'install.sh'), 'desktop'],
                                env=dict(self.env, PATH=str(binaries) + ':' + os.environ['PATH']),
                                input='si\n', capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 55, result.stdout + result.stderr)
        self.assertFalse((self.home / '.config/hypr').exists())
        self.assertNotIn('Configuración instalada', result.stdout)


if __name__ == '__main__':
    unittest.main()

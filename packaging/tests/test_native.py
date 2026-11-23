"""Native metadata checks use small fixtures, not alleged VPN sessions."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
from types import SimpleNamespace
import unittest

SPEC = importlib.util.spec_from_file_location('native', Path(__file__).parents[1] / 'native.py')
NATIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NATIVE)


class NativeMetadata(unittest.TestCase):
    @unittest.skipUnless(shutil.which('zstd') and shutil.which('bsdtar'), 'native archive tools')
    def test_arch_editions_conflict_without_replacing_system_openvpn(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            for variant in ('system', 'bundled'):
                stage = base / f'vmate-1.2.4-linux-amd64-{variant}'
                (stage / 'usr/bin').mkdir(parents=True)
                (stage / 'usr/bin/vmate-desktop').write_text('fixture')
                args = SimpleNamespace(arch='amd64', version='1.2.4', variant=variant, output=base)
                NATIVE.arch(args, stage)
                package = base / (stage.name + '.pkg.tar.zst')
                info = subprocess.check_output(['bsdtar', '-xOf', str(package), '.PKGINFO'], text=True)
                self.assertEqual('depend = openvpn\n' in info, variant == 'system')
                self.assertIn('conflict = vmate', info)
                self.assertIn('depend = ttf-dejavu', info)
                self.assertIn('pkgver = 1.2.4-3', info)
                if variant == 'bundled':
                    self.assertIn('pkgname = vmate-bundled', info)
                listing = subprocess.check_output(['bsdtar', '-tvf', str(package)], text=True)
                self.assertIn('root', listing)

    @unittest.skipUnless(shutil.which('rpmbuild'), 'RPM build tool')
    def test_rpm_bundled_dependency_does_not_require_global_openvpn(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            stage = base / 'vmate-1.2.4-linux-amd64-bundled'
            for folder in ('bin', 'share/applications', 'share/icons/hicolor/scalable/apps',
                           'share/doc/vmate', 'share/licenses/vmate', 'lib/vmate/openvpn/bin'):
                (stage / 'usr' / folder).mkdir(parents=True)
                (stage / 'usr' / folder / 'fixture').write_text('fixture')
            args = SimpleNamespace(arch='amd64', version='1.2.4', variant='bundled', output=base)
            NATIVE.rpm(args, stage)
            package = base / (stage.name + '.rpm')
            result = subprocess.check_output(['rpm', '-qp', '--requires', str(package)], text=True)
            self.assertNotIn('openvpn', result.lower())
            self.assertIn('libwayland-client', result)
            self.assertNotIn('wayland-libs-client', result)
            name = subprocess.check_output(['rpm', '-qp', '--queryformat', '%{NAME}', str(package)], text=True)
            self.assertEqual(name, 'vmate-bundled')
            release = subprocess.check_output(['rpm', '-qp', '--queryformat', '%{RELEASE}', str(package)], text=True)
            self.assertEqual(release, '0.3.desktop')


if __name__ == '__main__':
    unittest.main()

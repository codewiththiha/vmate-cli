"""Verify package provenance across Ubuntu merged-/usr aliases."""
import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('vendor', Path(__file__).parents[1] / 'vendor-linux.py')
VENDOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VENDOR)


class PackageOwnership(unittest.TestCase):
    def test_merged_usr_alias_retains_architecture(self):
        source = Path('/usr/lib/vmate-provenance-fixture.so')

        def query(arguments, **kwargs):
            filename = arguments[-1]
            found = filename == '/lib/vmate-provenance-fixture.so'
            return subprocess.CompletedProcess(arguments, 0 if found else 1,
                f'libfixture:amd64: {filename}\n' if found else '', '')

        with patch.object(VENDOR.subprocess, 'run', side_effect=query) as run:
            self.assertEqual(VENDOR.package_owner(source), 'libfixture:amd64')
            self.assertLessEqual(run.call_count, 3)

    def test_unrelated_metadata_cannot_supply_a_notice(self):
        result = subprocess.CompletedProcess([], 0, 'unrelated: /tmp/other-library.so\n', '')
        with patch.object(VENDOR.subprocess, 'run', return_value=result):
            with self.assertRaises(RuntimeError):
                VENDOR.package_owner(Path('/usr/lib/vmate-provenance-fixture.so'))

    def test_missing_owner_fails_closed(self):
        result = subprocess.CompletedProcess([], 1, '', 'not found')
        with patch.object(VENDOR.subprocess, 'run', return_value=result):
            with self.assertRaises(RuntimeError):
                VENDOR.package_owner(Path('/usr/lib/vmate-provenance-fixture.so'))


if __name__ == '__main__':
    unittest.main()

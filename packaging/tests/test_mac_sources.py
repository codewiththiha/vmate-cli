"""Retain checksum-verified patches without accepting dynamic Ruby inputs."""
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('mac_sources', Path(__file__).parents[1] / 'mac-sources.py')
MAC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MAC)


class MacSources(unittest.TestCase):
    def test_external_patch_is_retained_at_verified_hash(self):
        data = b'patch fixture'
        digest = hashlib.sha256(data).hexdigest()
        recipe = f'  patch do\n    url "https://example.org/fix.patch"\n    sha256 "{digest}"\n  end\n'
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(MAC.urllib.request, 'urlopen', return_value=io.BytesIO(data)):
                MAC.retain_patches(recipe, Path(directory))
            self.assertEqual((Path(directory) / 'homebrew-0.patch').read_bytes(), data)

    def test_checksum_mismatch_rejects_patch(self):
        recipe = '  patch do\n    url "https://example.org/fix.patch"\n    sha256 "' + '0' * 64 + '"\n  end\n'
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(MAC.urllib.request, 'urlopen', return_value=io.BytesIO(b'wrong')):
                with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
                    MAC.retain_patches(recipe, Path(directory))

    def test_dynamic_patch_still_requires_review(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, 'Unreviewed'):
                MAC.retain_patches('  patch :DATA\n', Path(directory))


if __name__ == '__main__':
    unittest.main()

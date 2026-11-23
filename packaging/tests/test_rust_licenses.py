"""Verify pinned notice recovery and reject unreviewed source revisions."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('licenses', Path(__file__).parents[1] / 'rust_licenses.py')
LICENSES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LICENSES)


class RustNotices(unittest.TestCase):
    def fixture(self, root, revision):
        (root / '.cargo_vcs_info.json').write_text(json.dumps({'git': {'sha1': revision}}))
        return {'name': 'block2', 'license': 'MIT',
                'repository': 'https://github.com/madsmtm/objc2'}

    def test_reviewed_revision_retains_summary_and_original_grant(self):
        with tempfile.TemporaryDirectory() as directory:
            package = self.fixture(Path(directory), 'b4167b582b2f75f9a1be75495c41b765344fd03c')
            with patch.object(LICENSES, 'upstream', return_value=['Upstream MIT summary']):
                text = '\n'.join(LICENSES.texts(package, Path(directory)))
            self.assertIn('Upstream MIT summary', text)
            self.assertIn('Copyright (c) Steven Sheldon', text)
            self.assertIn('cfb199226661ec6e4939fcd7cd7531b3c5453076', text)

    def test_unreviewed_revision_still_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            package = self.fixture(Path(directory), 'a' * 40)
            with patch.object(LICENSES, 'upstream', return_value=['MIT summary only']):
                with self.assertRaisesRegex(RuntimeError, 'require review'):
                    LICENSES.texts(package, Path(directory))

    def test_line_wrapped_grant_is_not_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            package = self.fixture(Path(directory), 'a' * 40)
            with patch.object(LICENSES, 'upstream', return_value=[
                    'Permission is hereby granted, free of charge,\nto any person obtaining a copy']):
                self.assertTrue(LICENSES.texts(package, Path(directory)))


if __name__ == '__main__':
    unittest.main()

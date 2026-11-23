#!/usr/bin/env python3
"""Publish complete, checksum-verified experimental desktop artifacts only."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def main():
    directory = Path(sys.argv[1])
    tag = os.environ['RELEASE_TAG']
    if not re.fullmatch(r'desktop-v\d+\.\d+\.\d+-alpha\.\d+', tag):
        raise RuntimeError('Use a desktop-vX.Y.Z-alpha.N tag; stable CLI tags are forbidden')
    manifests = list(directory.glob('checksums-*.json'))
    if len(manifests) != 4:
        raise RuntimeError(f'Expected four platform manifests, got {manifests}')
    for manifest in manifests:
        for name, expected in json.loads(manifest.read_text()).items():
            path = directory / name
            if path.parent != directory or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise RuntimeError(f'Asset checksum mismatch: {name}')
    for platform, architecture in [('linux', 'amd64'), ('linux', 'arm64'),
                                   ('macos', 'x86_64'), ('macos', 'arm64')]:
        for edition in ('system', 'bundled'):
            pattern = f'vmate-*-{platform}-{architecture}-{edition}'
            extensions = ['.dmg'] if platform == 'macos' else ['.deb', '.rpm', '.tar.gz']
            if platform == 'linux' and architecture == 'amd64':
                extensions.append('.pkg.tar.zst')
            for extension in extensions:
                if len(list(directory.glob(pattern + extension))) != 1:
                    raise RuntimeError(f'Missing or ambiguous edition: {pattern + extension}')
    files = sorted(p for p in directory.iterdir() if p.is_file())
    (directory / 'SHA256SUMS').write_text(''.join(
        f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in files))
    notes = Path('docs/desktop-prerelease.md')
    existing = subprocess.run(['gh', 'release', 'view', tag, '--json', 'isDraft,targetCommitish'],
                              capture_output=True, text=True, check=False)
    if existing.returncode == 0:
        release = json.loads(existing.stdout)
        if not release['isDraft'] or release['targetCommitish'] != os.environ['GITHUB_SHA']:
            raise RuntimeError('Refusing to modify an existing published or unrelated release')
    else:
        subprocess.run(['gh', 'release', 'create', tag, '--target', os.environ['GITHUB_SHA'],
                        '--draft', '--prerelease', '--title', f'vmate Desktop — {tag}',
                        '--notes-file', str(notes)], check=True)
    subprocess.run(['gh', 'release', 'upload', tag, '--clobber',
                    *map(str, sorted(directory.iterdir()))], check=True)
    subprocess.run(['gh', 'release', 'edit', tag, '--draft=false', '--prerelease'], check=True)


if __name__ == '__main__':
    main()

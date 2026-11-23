"""Recover omitted upstream notices only at the crate's recorded Git commit."""
from functools import lru_cache
import json
from pathlib import Path
import urllib.error
import urllib.request


@lru_cache(maxsize=64)
def upstream(repository, revision):
    if not repository.startswith('https://github.com/') or len(revision) != 40:
        raise RuntimeError(f'Cannot verify upstream license provenance: {repository} {revision}')
    repository = repository.removeprefix('https://github.com/').rstrip('/').removesuffix('.git')
    notices = []
    for filename in ('LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE', 'LICENSE.txt',
                     'LICENCE', 'LICENCE.txt', 'LICENSE.md', 'COPYING', 'LICENSES/MIT.txt', 'LICENSES/Apache-2.0.txt', 'NOTICE'):
        url = f'https://raw.githubusercontent.com/{repository}/{revision}/{filename}'
        try:
            text = urllib.request.urlopen(url, timeout=30).read().decode()
        except urllib.error.HTTPError as error:
            if error.code == 404:
                continue
            raise
        notices.append(f'{url}\n{text}')
    return notices


def texts(package, root):
    notices = []
    for path in sorted(root.rglob('*')):
        if path.is_file() and path.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE')):
            if path.stat().st_size > 2_000_000:
                raise RuntimeError(f'Unexpected notice size: {path}')
            notices.append(f'{path.relative_to(root)}\n{path.read_text(errors="replace")}')
    if notices:
        return notices
    identity = root / '.cargo_vcs_info.json'
    if not identity.exists():
        raise RuntimeError(f'Missing source revision for {package["name"]}')
    revision = json.loads(identity.read_text())['git']['sha1']
    notices = upstream(package['repository'], revision)
    text = '\n'.join(notices)
    declared = package['license'] or ''
    permitted = ('MIT' in declared and 'Permission is hereby granted, free of charge' in text
                 or 'Apache-2.0' in declared and 'Apache License' in text)
    if not permitted:
        raise RuntimeError(f'Upstream notices require review: {package["name"]}')
    return notices

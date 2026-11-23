"""Recover omitted upstream notices only at the crate's recorded Git commit."""
from functools import lru_cache
import json
from pathlib import Path
import urllib.error
import urllib.request


@lru_cache(maxsize=64)
def upstream(repository, revision):
    repository = repository.replace('http://github.com/', 'https://github.com/', 1)
    if not repository.startswith('https://github.com/') or len(revision) != 40:
        raise RuntimeError(f'Cannot verify upstream license provenance: {repository} {revision}')
    repository = repository.removeprefix('https://github.com/').rstrip('/').removesuffix('.git')
    notices = []
    for filename in ('LICENSE-MIT', 'LICENSE-APACHE', 'LICENSE', 'LICENSE.txt',
                     'LICENCE', 'LICENCE.txt', 'LICENSE.md', 'COPYING', 'LICENSES/MIT.txt', 'LICENSES/Apache-2.0.txt', 'NOTICE', 'UNLICENSE'):
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
        if path.is_file() and path.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE', 'UNLICENSE')):
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
    if (package['repository'] == 'https://github.com/madsmtm/objc2'
            and revision in ('b4167b582b2f75f9a1be75495c41b765344fd03c',
                             '8852b424193ca41602281b3d7540d7c8ed51e49a',
                             '5237cbf081d04ef75a1f6a207b4018e7c1ae7438',
                             'd7d2fa23ceaa5e6096c923b081040e5d81b3b9df',
                             'e282618be4c3a3b9542957e0c8540e9588472ce8',
                             '7b1abfd750a2cacaea71d6a56ecfb83cb7de560b',
                             '8d214f5477365ffcbcbb7de058c86ed9a518efb7',
                             )):
        retained = Path(__file__).parent / 'licenses'
        notices += [(retained / name).read_text() for name in
                    ('objc2-provenance.md', 'objc2-MIT.txt')]
    if (package['repository'] == 'http://github.com/SSheldon/rust-dispatch'
            and revision == '82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748'
            and package['license'] == 'MIT' and package['authors'] == ['Steven Sheldon']):
        retained = Path(__file__).parent / 'licenses'
        notices += [(retained / name).read_text() for name in
                    ('dispatch-provenance.md', 'objc2-MIT.txt')]
    text = ' '.join('\n'.join(notices).split())
    declared = package['license'] or ''
    permitted = ('MIT' in declared and 'Permission is hereby granted, free of charge' in text
                 or 'Apache-2.0' in declared and 'Apache License' in text)
    if not permitted:
        raise RuntimeError(f'Upstream notices require review: {package["name"]}')
    return notices

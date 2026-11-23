#!/usr/bin/env python3
"""Retain matching Homebrew library source archives, recipes and license texts."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import urllib.request


def main():
    prefix = Path(sys.argv[1])
    formulae = json.loads(subprocess.check_output(['brew', 'info', '--json=v2',
                                                 'openssl@3', 'lzo', 'lz4'], text=True))['formulae']
    for formula in formulae:
        name, version = formula['name'], formula['versions']['stable']
        installed = [item['version'].split('_')[0] for item in formula['installed']]
        if version not in installed:
            raise RuntimeError(f'Stable source does not match installed {name}: {installed}')
        source = formula['urls']['stable']
        data = urllib.request.urlopen(source['url'], timeout=120).read()
        if hashlib.sha256(data).hexdigest() != source['checksum']:
            raise RuntimeError(f'Source checksum mismatch: {name}')
        destination = prefix / 'share/sources' / name
        destination.mkdir(parents=True, exist_ok=True)
        (destination / 'upstream.tar.gz').write_bytes(data)
        (destination / 'homebrew.json').write_text(json.dumps(formula, indent=2))
        installed_prefix = Path(subprocess.check_output(['brew', '--prefix', name], text=True).strip())
        recipe = installed_prefix / '.brew' / f'{name}.rb'
        if not recipe.exists():
            raise RuntimeError(f'Missing exact build recipe: {recipe}')
        recipe_text = recipe.read_text()
        if 'patch do' in recipe_text or 'patch :' in recipe_text:
            raise RuntimeError(f'External source patches need review: {name}')
        (destination / recipe.name).write_text(recipe_text)
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:*') as archive:
            notices = [item for item in archive if item.isfile() and
                       Path(item.name).name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE'))]
            if not notices:
                raise RuntimeError(f'No source notices: {name}')
            text = '\n'.join(item.name + '\n' + archive.extractfile(item).read().decode(errors='replace')
                             for item in notices)
        license_dir = prefix / 'share/licenses' / name
        license_dir.mkdir(parents=True, exist_ok=True)
        (license_dir / 'NOTICES.txt').write_text(text)
    libraries = []
    for library in sorted((prefix / 'lib').glob('*.dylib')):
        if not library.name.startswith(('libcrypto.', 'libssl.', 'liblzo2.', 'liblz4.')):
            raise RuntimeError(f'Unreviewed bundled library: {library}')
        libraries.append({'name': library.name, 'sha256': hashlib.sha256(library.read_bytes()).hexdigest()})
    (prefix / 'share/licenses/library-manifest.json').write_text(json.dumps(libraries, indent=2))
    (prefix / 'share/sources/BUILD.txt').write_text(
        'Homebrew formula metadata and recipes describe the library builds.\n'
        'Rebuild OpenVPN with packaging/build-openvpn.sh from the tagged app source.\n')


if __name__ == '__main__':
    main()

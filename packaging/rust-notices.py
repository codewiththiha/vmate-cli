#!/usr/bin/env python3
"""Retain Rust dependency notices and exact cached source archives for release."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import re
import rust_licenses
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    host = next(line.split(': ', 1)[1] for line in subprocess.check_output(
        ['rustc', '-vV'], text=True).splitlines() if line.startswith('host:'))
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked',
        '--format-version', '1', '--filter-platform', host], text=True))
    tree = subprocess.check_output(['cargo', 'tree', '--workspace', '--locked',
        '--edges', 'normal,build', '--target', host, '--prefix', 'none', '--format', '{p}'], text=True)
    selected = set(re.findall(r'^(\S+) v(\S+)', tree, flags=re.MULTILINE))
    lock = tomllib.loads(Path('Cargo.lock').read_text())
    checksums = {(p['name'], p['version']): p.get('checksum') for p in lock['package']}
    inventory, notices = [], []
    cargo = Path.home() / '.cargo/registry/cache'
    with tempfile.TemporaryDirectory() as temporary:
        sources = Path(temporary)
        for package in metadata['packages']:
            if (package['name'], package['version']) not in selected or not (package['source'] or '').startswith('registry+'):
                continue
            name = f'{package["name"]}-{package["version"]}'
            archives = list(cargo.glob(f'*/{name}.crate'))
            if len(archives) != 1:
                raise RuntimeError(f'Exact source archive unavailable: {name}')
            archive = archives[0]
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            if digest != checksums.get((package['name'], package['version'])):
                raise RuntimeError(f'Crate source does not match Cargo.lock: {name}')
            (sources / archive.name).write_bytes(archive.read_bytes())
            inventory.append({'name': name, 'license': package['license'],
                              'source': package['source'],
                              'sha256': hashlib.sha256(archive.read_bytes()).hexdigest()})
            root = Path(package['manifest_path']).parent
            texts = rust_licenses.texts(package, root)
            notices.append(f'\n{name}\nDeclared license: {package["license"]}\n' + '\n'.join(texts))
        (sources / 'inventory.json').write_text(json.dumps(inventory, indent=2))
        with tarfile.open(args.output / f'rust-sources-{host}.tar.gz', 'w:gz') as output:
            output.add(sources, arcname='rust-sources')
    (args.output / 'THIRD-PARTY-NOTICES.txt').write_text('Rust dependency notices\n' + '\n'.join(notices))
    (args.output / 'rust-inventory.json').write_text(json.dumps(inventory, indent=2))
    print(f'Retained notices and exact sources for {len(inventory)} crates')


if __name__ == '__main__':
    main()

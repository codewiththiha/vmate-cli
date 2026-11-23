#!/usr/bin/env python3
"""Check tracked source ceilings and duplicate Rust bodies heuristically."""
import argparse
import hashlib
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def functions(text):
    pattern = r'(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)[^{;]*\{'
    for match in re.finditer(pattern, text):
        start = match.end() - 1
        depth = 1
        end = start + 1
        while end < len(text) and depth:
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        body = text[start:end]
        yield match.group(1), body.count('\n') + 1, body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dups', action='store_true')
    args = parser.parse_args()
    baseline = tomllib.loads((ROOT / 'tools/shape_baseline.toml').read_text())
    paths = subprocess.check_output(['git', 'ls-files'], cwd=ROOT, text=True).splitlines()
    errors, bodies = [], {}
    for name in paths:
        path = ROOT / name
        if path.suffix not in {'.rs', '.py', '.md', '.yml', '.toml', '.sh', '.slint'}:
            continue
        if name in baseline.get('exempt', {}):
            continue
        text = path.read_text()
        limit = baseline.get('files', {}).get(name, 500)
        if len(text.splitlines()) > limit:
            errors.append(f'{name}: exceeds {limit} lines')
        if path.suffix != '.rs':
            continue
        occurrences = {}
        for function, size, body in functions(text):
            count = occurrences.get(function, 0)
            occurrences[function] = count + 1
            key = f'{name}::{function}#{count}'
            ceiling = baseline.get('functions', {}).get(key, 80)
            if size > ceiling:
                errors.append(f'{key}: {size} > {ceiling} lines')
            normalized = re.sub(r'\s+', '', body)
            if args.dups and size > 10:
                digest = hashlib.sha256(normalized.encode()).hexdigest()
                if digest in bodies:
                    pair = f'{bodies[digest]} | {key}'
                    if pair not in baseline.get('duplicates', {}):
                        errors.append(f'duplicate body: {pair}')
                else:
                    bodies[digest] = key
    print('\n'.join(errors) if errors else 'Shape checks passed')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())

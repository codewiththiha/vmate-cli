#!/usr/bin/env python3
"""Check ordinary Rust comment lines in explicitly supplied files."""
import sys
from pathlib import Path


def check(path):
    errors = []
    for number, line in enumerate(path.read_text().splitlines(), 1):
        text = line.strip()
        if text.startswith('//') and not text.startswith(('///', '//!')):
            if len(line) > 80 or len(text[2:].split()) > 15:
                errors.append(f'{path}:{number}: ordinary comment exceeds budget')
            if any(word in text for word in ('TODO', 'FIXME', 'allow marker')):
                errors.append(f'{path}:{number}: forbidden comment marker')
        if '/*' in line:
            errors.append(f'{path}:{number}: review block comment manually')
    return errors


if __name__ == '__main__':
    if len(sys.argv) == 1:
        sys.exit('pass changed Rust files explicitly')
    failures = [error for name in sys.argv[1:] for error in check(Path(name))]
    print('\n'.join(failures) if failures else 'Comment checks passed')
    sys.exit(bool(failures))

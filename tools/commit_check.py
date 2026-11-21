#!/usr/bin/env python3
"""Enforce 50-character subjects and bodies for changes over 50 lines."""
import argparse
import subprocess
import sys
from pathlib import Path


def check(message, diff):
    lines = message.strip().splitlines()
    errors = []
    if not lines or not 1 <= len(lines[0]) <= 50:
        errors.append('subject must contain 1–50 characters')
    changed = 0
    for row in diff.splitlines():
        columns = row.split('\t')
        if len(columns) >= 3:
            changed += sum(int(n) for n in columns[:2] if n.isdigit())
    body = '\n'.join(lines[2:]).lower()
    if changed > 50:
        for label in ('what:', 'why:', 'verification:', 'search:'):
            if label not in body:
                errors.append(f'large commit body requires {label}')
    if lines and lines[0].startswith('perf'):
        for label in ('command:', 'machine:', 'before:', 'after:'):
            if label not in body:
                errors.append(f'performance commit requires {label}')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('message_file', nargs='?')
    parser.add_argument('--range')
    args = parser.parse_args()
    git = lambda *cmd: subprocess.check_output(['git', *cmd], text=True)
    errors = []
    if args.range:
        for commit in git('rev-list', '--no-merges', args.range).splitlines():
            errors += check(git('show', '-s', '--format=%B', commit),
                            git('diff-tree', '--root', '--no-commit-id', '--numstat', '-r', commit))
    elif args.message_file:
        errors = check(Path(args.message_file).read_text(), git('diff', '--cached', '--numstat'))
    else:
        parser.error('provide message_file or --range')
    print('\n'.join(errors) if errors else 'Commit checks passed')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())

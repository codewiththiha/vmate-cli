#!/usr/bin/env python3
"""Check added ordinary comment counts in a diff for explicit files."""
import argparse
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', default='HEAD')
    parser.add_argument('files', nargs='+')
    args = parser.parse_args()
    diff = subprocess.check_output(
        ['git', 'diff', '--unified=0', args.base, '--', *args.files], text=True)
    added = [line[1:] for line in diff.splitlines()
             if line.startswith('+') and not line.startswith('+++')]
    comments = sum(line.strip().startswith('//')
                   and not line.strip().startswith(('///', '//!')) for line in added)
    limit = max(2, len(added) // 25)
    print(f'Ordinary comments: {comments}/{limit}')
    return comments > limit


if __name__ == '__main__':
    sys.exit(main())

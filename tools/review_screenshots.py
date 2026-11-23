#!/usr/bin/env python3
"""Verify screenshot coverage and create a self-contained review gallery."""
from pathlib import Path
import base64
import sys

SCENARIOS = ['connected-library', 'about', 'library', 'relay-details', 'setup-bottom', 'empty', 'scanning', 'connection', 'connected', 'setup-linux',
             'setup-macos', 'missing-openvpn', 'permission-denied', 'activity', 'preferences']


def main():
    directory = Path(sys.argv[1])
    images = []
    for scenario in SCENARIOS:
        for size in ('desktop', 'compact'):
            path = directory / f'{scenario}-{size}.png'
            assert path.stat().st_size > 5000, f'missing or suspiciously blank screenshot: {path}'
            encoded = base64.b64encode(path.read_bytes()).decode()
            images.append(f'<section><h2>{scenario} · {size}</h2><img alt="{scenario} {size}" src="data:image/png;base64,{encoded}"></section>')
    html = '''<!doctype html><meta charset="utf-8"><title>vmate · Slint review</title>
<style>body{font:15px system-ui;background:#e9ebe5;color:#222c28;margin:32px}h1{font-size:28px}section{margin:36px 0}h2{font-size:16px}img{max-width:100%;box-shadow:0 3px 20px #0002}p{max-width:800px;line-height:1.6}</style>
<h1>vmate · desktop review</h1><p>Actual Slint software-renderer captures with deterministic fixtures. No network operations, no real connection claims. Native macOS chrome is outside this headless capture. Screen images supplement, not replace, engine lifecycle tests.</p>'''
    (directory.parent / 'gallery.html').write_text(html + ''.join(images))
    print(f'Coverage passed: {len(images)} rendered states; gallery.html generated')


if __name__ == '__main__':
    main()

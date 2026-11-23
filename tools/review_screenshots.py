#!/usr/bin/env python3
"""Verify screenshot coverage and create a self-contained review gallery."""
from pathlib import Path
import base64
import sys

SCENARIOS = ['home-idle', 'home-scanning', 'home-connecting', 'home-connected', 'home-retrying', 'home-stopping', 'home-exhausted', 'home-permission', 'connected-library', 'about', 'library', 'relay-details', 'setup-bottom', 'empty', 'scanning', 'connection', 'connected', 'setup-linux',
             'setup-macos', 'missing-openvpn', 'permission-denied', 'activity', 'preferences', 'preferences-bottom']


def main():
    directory = Path(sys.argv[1])
    images = []
    for scenario in SCENARIOS:
        for size in ('minimum', 'compact', 'desktop', 'wide'):
            for theme, suffix in [('light', ''), ('dark', '-dark')]:
                path = directory / f'{scenario}-{size}{suffix}.png'
                assert path.stat().st_size > 5000, f'missing or suspiciously blank screenshot: {path}'
                encoded = base64.b64encode(path.read_bytes()).decode()
                images.append(f'<section data-state="{scenario}" data-size="{size}"><h2>{scenario} · {size} · {theme}</h2><img alt="{scenario} {size}" src="data:image/png;base64,{encoded}"></section>')
    html = '''<!doctype html><meta charset="utf-8"><title>vmate · Slint review</title>
<style>body{font:15px system-ui;background:#e9ebe5;color:#222c28;margin:32px}h1{font-size:28px}section{margin:36px 0}h2{font-size:16px}img{max-width:100%;box-shadow:0 3px 20px #0002}p{max-width:800px;line-height:1.6}</style>
<h1>vmate · desktop review</h1><p>Actual Slint software-renderer captures with deterministic fixtures. No network operations, no real connection claims. Native system chrome is outside this headless capture. Both light and dark palettes are covered. Screen images supplement, not replace, engine lifecycle tests.</p>'''
    options = ''.join(f'<option>{state}</option>' for state in SCENARIOS)
    controls = f'<nav><label>State <select id="state">{options}</select></label> <label>Size <select id="size"><option value="all">All sizes</option><option>minimum</option><option>compact</option><option>desktop</option><option>wide</option></select></label></nav>'
    script = """<style>nav{position:sticky;top:0;background:#e9ebe5;padding:16px 0;z-index:1}select{padding:8px;font:inherit}label{margin-right:20px}[hidden]{display:none}</style>
<script>const state=document.querySelector('#state'),size=document.querySelector('#size');function update(){document.querySelectorAll('section').forEach(s=>s.hidden=s.dataset.state!==state.value||(size.value!=='all'&&s.dataset.size!==size.value))}state.onchange=update;size.onchange=update;update();</script>"""
    (directory.parent / 'gallery.html').write_text(html + controls + ''.join(images) + script)
    print(f'Coverage passed: {len(images)} rendered states; gallery.html generated')


if __name__ == '__main__':
    main()

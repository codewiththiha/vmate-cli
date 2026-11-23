#!/usr/bin/env python3
"""Generate the macOS app icon from the shared vector mark."""
from pathlib import Path
import subprocess
import sys
import tempfile
import cairosvg

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory() as temporary:
    iconset = Path(temporary) / 'vmate.iconset'
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            suffix = '@2x' if scale == 2 else ''
            cairosvg.svg2png(url=str(root / 'apps/desktop/assets/vmate.svg'),
                            write_to=str(iconset / f'icon_{size}x{size}{suffix}.png'),
                            output_width=size * scale, output_height=size * scale)
    subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o',
                    str(Path(sys.argv[1]) / 'vmate.icns')], check=True)

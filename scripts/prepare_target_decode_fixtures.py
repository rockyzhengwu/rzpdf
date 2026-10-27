#!/usr/bin/env python3
"""Generate a project-authored JPEG with detail at several spatial scales."""
import hashlib
import io
import json
from pathlib import Path
from PIL import Image, ImageDraw
from prepare_web_region_fixtures import fixture
from prepare_webassembly_fixtures import stream


def main():
    out = Path(__file__).resolve().parents[1] / 'output/webassembly/target-decode-fixtures'
    out.mkdir(parents=True, exist_ok=True)
    image = Image.new('RGB', (1600, 1200), 'white')
    draw = ImageDraw.Draw(image)
    for y in range(1200):
        draw.line((0, y, 1599, y), fill=(y * 255 // 1200, 120, 255 - y * 255 // 1200))
    for x in range(0, 1600, 16):
        draw.rectangle((x, 0, x + 7, 399), fill=(0, 0, 0))
    draw.ellipse((200, 400, 1400, 1100), fill='white', outline='black', width=8)
    buf = io.BytesIO()
    image.save(buf, format='JPEG', quality=90, progressive=False)
    data = fixture('/XObject << /I 5 0 R >>', 'q 200 0 0 150 0 0 cm /I Do Q',
                   extra=(stream('/Subtype /Image /Width 1600 /Height 1200 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode', buf.getvalue()),), size=(200, 150))
    (out / 'fit.pdf').write_bytes(data)
    (out / 'manifest.json').write_text(json.dumps(dict(license='AGPL-3.0-only', file='fit.pdf', sha256=hashlib.sha256(data).hexdigest()), indent=2) + '\n')
    print(out)


if __name__ == '__main__':
    main()

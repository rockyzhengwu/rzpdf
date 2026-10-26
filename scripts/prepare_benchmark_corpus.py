#!/usr/bin/env python3
"""Create deterministic, redistributable synthetic PDFs (no private documents)."""
import argparse
import hashlib
import io
import json
from pathlib import Path


def generate(directory):
    import fitz
    from PIL import Image, ImageDraw
    directory.mkdir(parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[1]
    font = root / "font-data/fonts/n019003l.pfb"
    cases = []
    for name in ("blank", "text-sparse", "text-dense", "text-multipage", "rectangles",
                 "curves", "transparency", "jpeg-small", "jpeg-scan", "flate-image",
                 "ttf-text", "cjk-sparse", "cjk-dense", "cjk-multipage"):
        doc = fitz.open()
        for index in range(5 if name.endswith("multipage") else 1):
            page = doc.new_page(width=600, height=800)
            if name.startswith("cjk") or name == "ttf-text":
                # Use only glyphs present in this redistributable, fixed subset.
                page.insert_font(fontname="BenchCJK", fontfile=str(root / "pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"))
                text = "Hello PDF " * 6 if name == "ttf-text" else "你好世界测试字符 " * 5
                for line in range(65 if name == "cjk-dense" else 20):
                    page.insert_text((30, 35 + line * 11), text, fontsize=10, fontname="BenchCJK")
            elif name.startswith("text"):
                page.insert_font(fontname="Bench", fontfile=str(font))
                for line in range(65 if name == "text-dense" else 20):
                    page.insert_text((30, 35 + line * 11),
                                     f"Page {index + 1}: PDF benchmark line {line:02d} - abcdef 0123456789",
                                     fontsize=10, fontname="Bench")
            elif name == "rectangles":
                shape = page.new_shape()
                for i in range(1500):
                    x, y = 20 + (i % 30) * 18, 20 + (i // 30) * 15
                    shape.draw_rect((x, y, x + 12, y + 10))
                shape.finish(color=(0.1, 0.3, 0.7), width=0.5)
                shape.commit()
            elif name == "curves":
                shape = page.new_shape()
                for i in range(1000):
                    x, y = 20 + (i % 25) * 22, 20 + (i // 25) * 18
                    shape.draw_bezier((x, y), (x + 20, y - 8), (x - 5, y + 20), (x + 15, y + 12))
                shape.finish(color=(0.15, 0.2, 0.6), width=0.7)
                shape.commit()
            elif name == "transparency":
                for i in range(40):
                    x, y = 30 + i * 9, 40 + i * 13
                    page.draw_rect((x, y, x + 150, y + 150), color=None,
                                   fill=((i % 3) / 2, ((i + 1) % 3) / 2, 0.4), fill_opacity=0.35)
            elif "image" in name or "jpeg" in name:
                size = (2500, 3500) if name == "jpeg-scan" else (1000, 1400)
                image = Image.new("RGB", size, "white")
                draw = ImageDraw.Draw(image)
                for y in range(0, size[1], 20):
                    draw.rectangle((20, y, size[0] - 20, y + 10), fill=(y % 256, 80, 160))
                stream = io.BytesIO()
                image.save(stream, "PNG" if name == "flate-image" else "JPEG")
                page.insert_image((20, 20, 580, 780), stream=stream.getvalue())
        path = directory / f"{name}.pdf"
        doc.save(path, deflate=True, no_new_id=True)
        doc.close()
        cases.append(dict(name=name, path=str(path.resolve()),
                          sha256=hashlib.sha256(path.read_bytes()).hexdigest(), nonblank=name != "blank"))
    manifest = directory / "manifest.json"
    manifest.write_text(json.dumps(dict(schema=1, generator="synthetic-v1", cases=cases), indent=2) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    print(generate(parser.parse_args().directory))

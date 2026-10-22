"""Generate our own tiny codec fixtures; no third-party image/PDF is copied.

Requires Pillow/OpenJPEG, pikepdf, opj_compress and optional jbig2enc.
Usage: PYTHONPATH=tmp/pdfs/v05-verifiers python3 pdf/tests/fixtures/v06/generate.py
  --jbig2 tmp/pdfs/v06-jbig2enc/build/jbig2
Outputs are intentionally reproducible inputs, not evidence of rzpdf correctness.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from PIL import Image
import pikepdf

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--jbig2", type=Path)
parser.add_argument("--refresh-generated", action="store_true")
args = parser.parse_args()
outputs = {}
old_manifest=json.loads((ROOT/"manifest.json").read_text()) if (ROOT/"manifest.json").exists() else {}

def store(name, data):
    target = ROOT / name
    if target.exists():
        if target.read_bytes() != data:
            if not args.refresh_generated or (name!="manifest.json" and hashlib.sha256(target.read_bytes()).hexdigest()!=old_manifest.get(name)):
                raise RuntimeError(f"refusing to replace changed fixture {target}")
            target.write_bytes(data)
    else:
        target.write_bytes(data)
    outputs[name] = hashlib.sha256(data).hexdigest()

rgb = Image.new("RGB", (16, 16))
rgb.putdata([(x * 17, y * 17, ((x + y) % 16) * 17) for y in range(16) for x in range(16)])
gray = Image.new("L", (16, 16))
gray.putdata([(x * 11 + y * 5) % 256 for y in range(16) for x in range(16)])
for name, image, options in [
    ("rgb-lossless.jp2", rgb, {}),
    ("gray-lossless.jp2", gray, {}),
    ("rgb-lossy.jp2", rgb, {"irreversible": True, "quality_mode": "rates", "quality_layers": [5]}),
    ("rgb-raw.j2k", rgb, {"no_jp2": True}),
]:
    buf = io.BytesIO()
    image.save(buf, "JPEG2000", **options)
    store(name, buf.getvalue())
store("rgb.samples", rgb.tobytes())
store("gray.samples", gray.tobytes())

# Independent OpenJPEG CLI producer, not Pillow or rzpdf.
with tempfile.TemporaryDirectory(prefix="rzpdf-v06-") as scratch:
    scratch = Path(scratch)
    gray.save(scratch / "gray.pgm")
    result = subprocess.run(["opj_compress", "-i", str(scratch / "gray.pgm"), "-o", str(scratch / "gray.j2k"), "-n", "3"], capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode())
    store("openjpeg-gray.j2k", (scratch / "gray.j2k").read_bytes())

def mmr(image):
    buf = io.BytesIO()
    # Pillow's TIFF writer uses PhotometricInterpretation=BlackIsZero. JBIG2
    # has the Group 4 white-run convention, so invert the TIFF sample carrier.
    image=image.convert("L").point(lambda pixel:255-pixel).convert("1")
    image.save(buf, "TIFF", compression="group4")
    buf.seek(0)
    tiff = Image.open(buf)
    offsets, sizes = tiff.tag_v2[273], tiff.tag_v2[279]
    if len(offsets) != 1:
        raise RuntimeError("expected one TIFF strip")
    return buf.getvalue()[offsets[0]:offsets[0] + sizes[0]]

def segment(number, kind, association, data, references=()):
    if len(references) > 4 or number > 256:
        raise ValueError("fixture uses compact segment headers")
    return struct.pack(">IBB", number, kind, len(references) << 5) + bytes(references) + bytes([association]) + struct.pack(">I", len(data)) + data

def region(width, height):
    return struct.pack(">IIIIB", width, height, 0, 0, 0)

def page_info(width, height):
    return struct.pack(">IIIIBH", width, height, 72, 72, 0, 0)

mono = Image.new("1", (32, 32))
mono.putdata([255 if (x // 4 + y // 4) % 2 else 0 for y in range(32) for x in range(32)])
store("mono.samples", mono.convert("L").tobytes())
generic = segment(1, 48, 1, page_info(32, 32)) + segment(2, 38, 1, region(32, 32) + b"\x01" + mmr(mono)) + segment(3, 49, 1, b"")
store("generic-mmr.jb2", generic)

# Meaningful shared Globals: two patterns selected by a halftone index plane.
# Both pages refer to the same pattern dictionary; there is no empty placeholder.
patterns = Image.new("1", (16, 8))
patterns.putdata([255 if ((x % 8) // 4 + y // 4 + x // 8) % 2 else 0 for y in range(8) for x in range(16)])
globals_data = segment(1, 16, 0, b"\x01\x08\x08" + struct.pack(">I", 1) + mmr(patterns))
store("patterns-global.jb2", globals_data)
planes = []
for page_number in (0, 1):
    plane = Image.new("1", (4, 4))
    plane.putdata([255 if (x + y + page_number) % 2 else 0 for y in range(4) for x in range(4)])
    planes.append(plane)
    header = region(32, 32) + b"\x01" + struct.pack(">IIiiHH", 4, 4, 0, 0, 2048, 0)
    data = segment(2, 48, 1, page_info(32, 32)) + segment(3, 22, 1, header + mmr(plane), (1,)) + segment(4, 49, 1, b"")
    store(f"halftone-{page_number}.jb2", data)

if args.jbig2:
    with tempfile.TemporaryDirectory(prefix="rzpdf-v06-jbig2-") as scratch:
        image_path = Path(scratch) / "our-checker.pbm"
        mono.save(image_path)
        result = subprocess.run([str(args.jbig2.resolve()), "-p", str(image_path)], capture_output=True)
        if result.returncode:
            raise RuntimeError(result.stderr.decode())
        store("jbig2enc-generic.jb2", result.stdout)

# A real PDF from libqpdf/pikepdf with the encoded sources, no rzpdf writer.
pdf = pikepdf.Pdf.new()
global_stream = pdf.make_stream(globals_data)
for name, width, height, bpc, color in [
    ("rgb-lossless.jp2", 16, 16, None, None),
    ("gray-lossless.jp2", 16, 16, None, None),
    ("rgb-lossy.jp2", 16, 16, 8, "/DeviceRGB"),
    ("rgb-raw.j2k", 16, 16, 8, "/DeviceRGB"),
    ("openjpeg-gray.j2k", 16, 16, 8, "/DeviceGray"),
    ("generic-mmr.jb2", 32, 32, 1, "/DeviceGray"),
    ("halftone-0.jb2", 32, 32, 1, "/DeviceGray"),
    ("halftone-1.jb2", 32, 32, 1, "/DeviceGray"),
] + ([("jbig2enc-generic.jb2", 32, 32, 1, "/DeviceGray")] if args.jbig2 else []):
    page = pdf.add_blank_page(page_size=(100, 100))
    image = pdf.make_stream((ROOT / name).read_bytes())
    image.Type = pikepdf.Name.XObject
    image.Subtype = pikepdf.Name.Image
    image.Width, image.Height = width, height
    image.Filter = pikepdf.Name.JPXDecode if name.endswith(("jp2", "j2k")) else pikepdf.Name.JBIG2Decode
    if bpc:
        image.BitsPerComponent = bpc
    if color:
        image.ColorSpace = pikepdf.Name(color)
    if name.startswith("halftone"):
        image.DecodeParms = pikepdf.Dictionary(JBIG2Globals=global_stream)
    page.Resources = pikepdf.Dictionary(XObject=pikepdf.Dictionary(Im=image))
    page.Contents = pdf.make_stream(b"q 64 0 0 64 18 18 cm /Im Do Q")
buf = io.BytesIO()
pdf.save(buf, deterministic_id=True, compress_streams=False, min_version="1.5")
store("external-codecs.pdf", buf.getvalue())
# The channel fixtures have a separate, pinned producer; retain verified entries.
for name in ("gray-alpha.jp2", "rgb-alpha.jp2", "cmyk-lossless.jp2", "gray-indices-2bit.jp2"):
    if name in old_manifest:
        digest = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        if digest != old_manifest[name]:
            raise RuntimeError(f"changed channel fixture: {name}")
        outputs[name] = digest
store("manifest.json", (json.dumps(outputs, indent=2, sort_keys=True) + "\n").encode())
print(json.dumps(outputs, indent=2, sort_keys=True))

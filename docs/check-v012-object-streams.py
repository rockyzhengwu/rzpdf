"""Offline V12-03 gate. Build the v012_layout example first.
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-object-streams.py NEW_DIRECTORY
Uses existing tools only; also runs the unchanged V12-02 gate in input-gate/.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pikepdf
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v012_layout"


def run(label, *args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


run("v012-02-regression", sys.executable, ROOT / "docs/check-v012-xref-output.py", OUT / "input-gate")
fixtures = OUT / "input-gate"
source = fixtures / "pikepdf-classic.pdf"
raw = OUT / "rzpdf-objects-raw.pdf"
flate = OUT / "rzpdf-objects-flate.pdf"
run("produce-raw", EXE, source, raw, "objects")
run("produce-flate", EXE, source, flate, "objects", "flate")
many = OUT / "pikepdf-many.pdf"
with pikepdf.open(source) as pdf:
    pdf.Root.RzMany = pikepdf.Array([pdf.make_indirect(pikepdf.Dictionary(
        Index=i, Label=pikepdf.String(f"member {i}"))) for i in range(230)])
    pdf.save(many, force_version="1.7", object_stream_mode=pikepdf.ObjectStreamMode.disable,
             compress_streams=False, fix_metadata_version=False)


def structural(obj):
    return isinstance(obj, pikepdf.Stream) and obj.get("/Type") in (
        pikepdf.Name.ObjStm, pikepdf.Name.XRef)


def projection(pdf):
    values, containers = {}, set()
    for obj in pdf.objects:
        if obj.objgen == (0, 0):
            continue
        if structural(obj):
            containers.add(obj.objgen)
            continue
        if isinstance(obj, pikepdf.Stream):
            dictionary = pikepdf.Dictionary(obj.stream_dict)
            for key in ("/Length", "/Filter", "/DecodeParms"):
                if key in dictionary:
                    del dictionary[key]
            values[obj.objgen] = (dictionary.unparse(resolved=True), obj.read_bytes())
        else:
            values[obj.objgen] = obj.unparse(resolved=True)
    return values, containers


def check_groups(pdf, label):
    size = int(pdf.trailer.Size)
    w = list(pdf.trailer.W)
    assert w == [1, 8, 4], (label, w)
    assert list(pdf.trailer.Index) == [0, size]
    assert not any(k in pdf.trailer for k in ("/Prev", "/XRefStm", "/Encrypt", "/Filter"))
    xrefs = [obj for obj in pdf.objects if isinstance(obj, pikepdf.Stream)
             and obj.get("/Type") == pikepdf.Name.XRef]
    assert len(xrefs) == 1
    rows = xrefs[0].read_bytes()
    assert len(rows) == size * 13
    groups, members = [], set()
    for obj in pdf.objects:
        if not isinstance(obj, pikepdf.Stream) or obj.get("/Type") != pikepdf.Name.ObjStm:
            continue
        payload = obj.read_bytes()
        n, first = int(obj.N), int(obj.First)
        assert 0 < n <= 100 and 0 < first < len(payload) <= 1024*1024
        assert "/Extends" not in obj
        header = [int(x) for x in payload[:first].split()]
        assert len(header) == 2*n and header[1] == 0
        offsets = header[1::2]
        assert offsets == sorted(set(offsets)) and first+offsets[-1] < len(payload)
        for index, number in enumerate(header[::2]):
            assert number not in members
            members.add(number)
            row = rows[number*13:(number+1)*13]
            assert row[0] == 2
            assert int.from_bytes(row[1:9], "big") == obj.objgen[0]
            assert int.from_bytes(row[9:], "big") == index
            member = pdf.get_object((number, 0))
            assert not isinstance(member, pikepdf.Stream)
        groups.append(dict(id=list(obj.objgen), count=n, first=first,
                           decoded_bytes=len(payload), encoded_bytes=len(obj.read_raw_bytes()),
                           flate=obj.get("/Filter") == pikepdf.Name.FlateDecode))
    compressed = {i for i in range(size) if rows[13*i] == 2}
    assert compressed == members and groups
    assert rows[pdf.Root.objgen[0]*13] == 1  # Catalog policy
    for obj in pdf.objects:
        if isinstance(obj, pikepdf.Stream):
            assert rows[obj.objgen[0]*13] == 1
    if "many" in label:
        assert len(groups) >= 3 and groups[0]["count"] == 100 and groups[1]["count"] == 100
    return groups


def render(label, path):
    folder = OUT / label
    folder.mkdir()
    run(label, "mutool", "draw", "-r", "96", "-o", folder / "page-%d.png", path)
    return folder


cases, pixels, thumbs = [], [], []
sources = [source, fixtures / "qpdf-objstm.pdf", fixtures / "mupdf-classic.pdf",
           fixtures / "rzpdf-stream.pdf", fixtures / "synthetic-hybrid.pdf", raw, flate, many]
for original in sources:
    run(original.stem + "-source-check", "qpdf", "--check", original)
    reference = render(original.stem + "-reference", original)
    for compression in (False, True):
        label = original.stem + ("-flate" if compression else "-preserve")
        output = OUT / (label + "-packed.pdf")
        run(label, EXE, original, output, "objects", *(["flate"] if compression else []))
        run(label + "-check", "qpdf", "--check", output)
        run(label + "-xref", "qpdf", "--show-xref", output)
        with pikepdf.open(original) as before, pikepdf.open(output) as after:
            expected, replaced = projection(before)
            actual, generated = projection(after)
            assert actual == expected, label
            assert not replaced.intersection(generated), label
            assert all(objgen[0] >= int(before.trailer.Size) for objgen in generated), label
            groups = check_groups(after, label)
            assert len(after.pages) == len(before.pages) == 2
        actual_render = render(label + "-actual", output)
        for page in (1, 2):
            a = Image.open(reference / f"page-{page}.png").convert("RGB")
            b = Image.open(actual_render / f"page-{page}.png").convert("RGB")
            assert a.size == b.size and ImageChops.difference(a, b).getbbox() is None, (label, page)
            pixels.append(dict(case=label, page=page, exact_pixels=True))
            thumbs.append((label, page, b.copy()))
        cases.append(dict(case=label, source=str(original.relative_to(OUT)), output=output.name,
                          replaced_containers=sorted(replaced), groups=groups))

# Semantic stream collections and active owner references are not garbage.
for case in ("extends", "unknown-field", "active-reference", "trailer-reference"):
    original = OUT / (case + ".pdf")
    with pikepdf.open(source) as pdf:
        old = pdf.make_stream(b"")
        old.Type, old.N, old.First = pikepdf.Name.ObjStm, 0, 0
        # An ordinary owner keeps this deliberately constructed container alive
        # while qpdf writes the input. The tests distinguish each refusal cause.
        pdf.Root.RzContainer = old
        if case == "extends":
            old.Extends = old
        elif case == "unknown-field":
            old.RzSemantic = pikepdf.String("preserve me")
        elif case == "trailer-reference":
            pdf.trailer.RzContainer = old
        pdf.save(original, force_version="1.7", object_stream_mode=pikepdf.ObjectStreamMode.disable,
                 compress_streams=False, fix_metadata_version=False)
    refused = OUT / (case + "-refused.pdf")
    run(case, EXE, original, refused, "objects", success=False)
    assert not refused.exists()

linearized = fixtures / "linearized.pdf"
ordinary = OUT / "linearized-to-ordinary.pdf"
run("linearized-rewrite", EXE, linearized, ordinary, "objects")
assert "File is not linearized" in run("ordinary-check", "qpdf", "--check", ordinary)
with pikepdf.Pdf.open(ordinary) as after, pikepdf.Pdf.open(linearized) as before:
    assert not after.is_linearized
    assert len(after.pages) == len(before.pages)
    for a, b in zip(after.pages, before.pages):
        assert a.Contents.read_bytes() == b.Contents.read_bytes()
existing = OUT / "existing.pdf"
existing.write_bytes(b"keep existing target")
run("no-overwrite", EXE, source, existing, "objects", success=False)
assert existing.read_bytes() == b"keep existing target"

# Compact contact sheets retain every latest output page for visual QA.
for sheet in range(2):
    selected = thumbs[sheet*16:(sheet+1)*16]
    canvas = Image.new("RGB", (1760, 1400), "#eeeeee")
    draw = ImageDraw.Draw(canvas)
    for index, (label, page, picture) in enumerate(selected):
        x, y = (index % 4)*440, (index // 4)*350
        picture.thumbnail((426, 320))
        canvas.paste(picture, (x+7, y+24))
        draw.text((x+7, y+5), f"{label} / {page}", fill="black")
    canvas.save(OUT / f"visual-review-{sheet+1}.png")

(OUT / "manifest.json").write_text(json.dumps(dict(cases=cases, pixels=pixels,
    versions=dict(pikepdf=pikepdf.__version__, qpdf=run("qpdf-version", "qpdf", "--version"),
                  mupdf=run("mupdf-version", "mutool", "-v")),
    hashes={str(p.relative_to(OUT)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in OUT.rglob("*.pdf")},
    provenance="Synthetic pikepdf; qpdf ObjStm; MuPDF classic; rzpdf xref/raw/Flate ObjStm; explicit hybrid. No network/install/fuzz/signing.",
    scope="Business IDs/values preserved; only proven structural containers replaced. Extends/unknown fields/active refs refuse."), indent=2) + "\n")
print(f"PASS {len(cases)} packed rewrites, {len(pixels)} exact MuPDF page pairs; V12-02 gate also passed")

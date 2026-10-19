"""Original AF producer preflight, AGPL-3.0-only. Existing local tools only.
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v014-producers.py NEW_DIR
This validates producer capability, not v0.14 engine acceptance.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import pikepdf

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
PAYLOAD = b"AF fixture\n"


def run(label, *args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=120)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.stderr)
    return result.stdout + result.stderr


with pikepdf.Pdf.new() as pdf:
    stream = pdf.make_stream(PAYLOAD)
    stream.Type = pikepdf.Name.EmbeddedFile
    stream.Subtype = pikepdf.Name("/text/plain")
    spec = pdf.make_indirect(pikepdf.Dictionary(
        Type=pikepdf.Name.Filespec, F=pikepdf.String("source.txt"), UF=pikepdf.String("source.txt"),
        AFRelationship=pikepdf.Name.Source,
        EF=pikepdf.Dictionary(F=stream, UF=stream),
        Desc=pikepdf.String("Original pikepdf synthetic fixture"),
    ))
    pdf.Root.AF = pdf.make_indirect(pikepdf.Array([spec]))
    pdf.Root.OriginalAF = pdf.Root.AF
    font = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1, BaseFont=pikepdf.Name.Helvetica))
    for _ in range(2):
        page = pdf.add_blank_page(page_size=(200, 200))
        page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=font))
        page.Contents = pdf.make_stream(b"q 0 g 10 10 20 20 re f Q\nBT /F1 12 Tf 30 100 Td (AF fixture text) Tj ET\n")
        page.AF = pikepdf.Array([spec])
    pdf.save(OUT / "pikepdf.pdf", force_version="2.0")

run("mupdf-create", "mutool", "run", ROOT / "docs/fixtures-v014-associated-files.js", OUT / "mupdf.pdf")
records = []
for producer in ["pikepdf", "mupdf"]:
    source = OUT / (producer + ".pdf")
    run(producer + "-check", "qpdf", "--check", source)
    with pikepdf.open(source, attempt_recovery=False) as pdf:
        assert len(pdf.Root.AF) == 1 and len(pdf.pages) == 2
        spec = pdf.Root.AF[0]
        assert str(spec.AFRelationship) == "/Source"
        stream = spec.EF.UF if "/UF" in spec.EF else spec.EF.F
        assert str(stream.Subtype) == "/text/plain"
        assert stream.read_bytes() == PAYLOAD
        for page in pdf.pages:
            assert page.AF[0].objgen == spec.objgen
        assert "/Names" not in pdf.Root
    run(producer + "-render", "mutool", "draw", "-q", "-r", "72", "-o", OUT / (producer + "-%d.png"), source)
    records.append(dict(producer=producer, sha256=hashlib.sha256(source.read_bytes()).hexdigest(), pages=2))

manifest = dict(license="AGPL-3.0-only", stage="producer preflight only", records=records,
                payload_sha256=hashlib.sha256(PAYLOAD).hexdigest(),
                versions=dict(pikepdf=pikepdf.__version__, qpdf=run("qpdf-version", "qpdf", "--version"),
                              mupdf=run("mupdf-version", "mutool", "-v")))
(OUT / "manifest.json").write_text(json.dumps(manifest, indent=2))
print(json.dumps(manifest, indent=2))

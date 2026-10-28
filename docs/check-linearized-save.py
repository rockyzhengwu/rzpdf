"""Offline interoperability gate; requires qpdf and PyMuPDF, no downloads.

cargo build --locked -p pdf --example v012_layout
python3 docs/check-linearized-save.py output/linearized-save-NEW
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import fitz

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v012_layout"


def run(label, *args):
    result = subprocess.run(
        [str(arg) for arg in args], capture_output=True, text=True, timeout=120
    )
    (OUT / f"{label}.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


records = []
for name in ("two-pages", "two-pages-objects", "two-pages-overflow"):
    source = ROOT / "pdf/tests/fixtures/linearized" / f"{name}.pdf"
    assert "File is linearized" in run(name + "-source", "qpdf", "--check", source)
    for layout in ("classic", "stream", "objects"):
        label = f"{name}-{layout}"
        output = OUT / f"{label}.pdf"
        run(label + "-save", EXE, source, output, layout)
        assert "File is not linearized" in run(label + "-check", "qpdf", "--check", output)
        assert b"/Linearized" not in output.read_bytes()
        comparisons = []
        with fitz.open(source) as before, fitz.open(output) as after:
            assert len(before) == len(after) == 2
            for dpi in (72, 144):
                for index in range(len(before)):
                    a = before[index].get_pixmap(dpi=dpi, alpha=True)
                    b = after[index].get_pixmap(dpi=dpi, alpha=True)
                    assert (a.width, a.height) == (b.width, b.height)
                    assert a.samples == b.samples, (label, dpi, index)
                    comparisons.append(dict(page=index, dpi=dpi, exact_pixels=True))
        records.append(dict(
            source=source.name, layout=layout, output=output.name,
            source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
            output_sha256=hashlib.sha256(output.read_bytes()).hexdigest(),
            comparisons=comparisons,
        ))

(OUT / "report.json").write_text(json.dumps(dict(
    qpdf=run("qpdf-version", "qpdf", "--version").strip(),
    pymupdf=fitz.VersionBind, records=records,
), indent=2) + "\n")
print("PASS: 9 ordinary rewrites checked by qpdf; 36 exact MuPDF page comparisons")

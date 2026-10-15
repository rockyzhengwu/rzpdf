"""Recheck v0.3 real-file and static subset baselines on the v0.7 candidate.

Needs v03_probe, v03_font_matrix, v03_subset_matrix, render_document, Python
3.11 with PyMuPDF/FontTools/Pillow, qpdf, mutool. Original public files are
not redistributed; source/license links and hashes are in v0.3-support.md.
Pass a directory with cantonese-wikipedia.pdf and scanner-test.pdf, or use
the existing local output/pdf/v03-external directory. Never replaces evidence.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import fitz
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "output/pdf/v03-external"
OUT = Path(tempfile.mkdtemp(prefix="v07-font-regressions-", dir=ROOT / "tmp/pdfs"))


def run(*args, fail=False):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True)
    if (result.returncode != 0) != fail:
        raise RuntimeError(f"unexpected exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout + result.stderr


reports = []
for name, digest, mode in [
    ("cantonese-wikipedia", "c021c80a85cbe19bccd19aa538e93f9490873aea6600676e8fbf6b2d41ced143", "--compat"),
    ("scanner-test", "7e74fb2a07678dfea62ab7d773a2cd3c78e6fd5fe81b379d9ccde7303e3187db", "--strict"),
]:
    original, saved = SOURCE / f"{name}.pdf", OUT / f"{name}.pdf"
    assert hashlib.sha256(original.read_bytes()).hexdigest() == digest
    strict_status = "passed"
    if mode == "--compat":
        # Historical v0.3 rejected this input's free entry. Later parser work
        # may accept it strictly; measure the current result, not the old claim.
        strict = subprocess.run([str(ROOT / "target/debug/examples/v03_probe"), str(original), str(OUT / "strict-check.pdf"), "--strict"], capture_output=True, text=True)
        (OUT / "strict-check.log").write_text(strict.stdout + strict.stderr)
        if strict.returncode == 0:
            assert "roundtrip=strict-ok" in strict.stdout
            mode = "--strict"
        else:
            assert "free" in strict.stderr, strict.stderr
            strict_status = "historical free-entry rejection; compatible mode required"
    probe = run(ROOT / "target/debug/examples/v03_probe", original, saved, mode)
    (OUT / f"{name}-probe.log").write_text(probe)
    assert "roundtrip=strict-ok" in probe
    assert "diagnostics=0" in probe
    qpdf = run("qpdf", "--check", saved)
    for path, tag in [(original, "original"), (saved, "saved")]:
        run("mutool", "draw", "-q", "-r", "72", "-o", OUT / f"{name}-{tag}-%d.png", path)
    # Skia's standalone driver is deliberately strict. The Chinese original
    # has a known malformed free entry, so render the strict saved copy.
    run(ROOT / "target/debug/examples/render_document", saved, OUT / f"{name}-skia")
    with fitz.open(original) as a, fitz.open(saved) as b:
        assert len(a) == len(b)
        for page in range(len(a)):
            assert a[page].get_text("rawdict") == b[page].get_text("rawdict"), (name, page)
            first = Image.open(OUT / f"{name}-original-{page+1}.png").convert("RGB")
            second = Image.open(OUT / f"{name}-saved-{page+1}.png").convert("RGB")
            assert ImageChops.difference(first, second).getbbox() is None, (name, page)
        reports.append(dict(name=name, sha256=digest, pages=len(a), strict_status=strict_status, probe=probe, qpdf=qpdf))

for kind, checker in [("font_matrix", "check-v03-font-matrix.py"), ("subset_matrix", "check-v03-font-subset.py")]:
    path = OUT / f"{kind}.pdf"
    run(ROOT / f"target/debug/examples/v03_{kind}", path)
    check = run(sys.executable, ROOT / "docs" / checker, path)
    run(ROOT / "target/debug/examples/render_document", path, OUT / f"{kind}-skia")
    run("mutool", "draw", "-q", "-r", "72", "-o", OUT / f"{kind}-mupdf-%d.png", path)
    reports.append(dict(name=kind, check=check, qpdf=run("qpdf", "--check", path)))

report = dict(head=run("git", "-C", ROOT, "rev-parse", "HEAD").strip(), status=run("git", "-C", ROOT, "status", "--short").strip(), reports=reports)
(OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
print(OUT / "report.json")

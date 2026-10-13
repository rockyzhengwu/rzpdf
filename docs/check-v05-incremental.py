"""Independent v05-A checks. Requires example, qpdf, MuPDF, pypdf and pikepdf.

Usage: PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-incremental.py
Creates a fresh evidence directory; does not overwrite previous evidence.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from pypdf import PdfReader
import pikepdf

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = ROOT / "target/debug/examples/v05_incremental"
FIXTURE = ROOT / "pdf/tests/fixtures/v04/reportlab.pdf"
EVIDENCE = Path(tempfile.mkdtemp(prefix="v05-incremental-", dir=ROOT / "tmp/pdfs"))
LEGACY_WARNINGS = []


def run(*args, succeeds=True):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True)
    if (result.returncode == 0) != succeeds:
        raise RuntimeError(f"unexpected exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()


def render(path, prefix):
    run("mutool", "draw", "-q", "-r", "72", "-o", EVIDENCE / f"{prefix}-%d.png", path)
    return [file.read_bytes() for file in sorted(EVIDENCE.glob(f"{prefix}-*.png"))]


def check_pdf(path, deleted_compressed=None):
    with pikepdf.Pdf.open(path, attempt_recovery=False) as reader:
        warnings = reader.check_pdf_syntax()
        if warnings:
            raise AssertionError(f"libqpdf {pikepdf.__libqpdf_version__}: {warnings}")
        if deleted_compressed and reader.get_object(deleted_compressed) is not None:
            raise AssertionError("deleted compressed object still resolves")
    result = subprocess.run(["qpdf", "--check", str(path)], capture_output=True, text=True)
    if result.returncode == 0:
        return
    # qpdf 11.2 inserts a default entry while looking up a deleted ObjStm
    # member (QPDF.cc::resolveObjectsInStream uses operator[]). The newer
    # libqpdf check above must pass without recovery or warnings. Record this
    # exact legacy diagnostic, not a blanket allowance for exit status 3.
    warning_lines = [line for line in result.stderr.splitlines() if line.startswith("WARNING:")]
    if deleted_compressed and result.returncode == 3 and run("qpdf", "--version").startswith("qpdf version 11.2."):
        number, generation = deleted_compressed
        expected = f"object {number}/{generation} has unexpected xref entry type"
        if len(warning_lines) == 1 and warning_lines[0].endswith(expected):
            LEGACY_WARNINGS.append({"file": str(path), "exit": result.returncode, "stderr": result.stderr.strip()})
            return
    raise RuntimeError(f"qpdf check exit {result.returncode}: {path}\n{result.stdout}\n{result.stderr}")


def verify_chain(source, label):
    source_bytes = source.read_bytes()
    baseline = render(source, f"{label}-original")
    for index in range(3):
        target = EVIDENCE / f"{label}-{index}.pdf"
        run(EXAMPLE, source, target, f"Incremental {index}")
        output = target.read_bytes()
        if not output.startswith(source_bytes):
            raise AssertionError("original prefix changed")
        previous = int(source_bytes.rsplit(b"startxref", 1)[1].split()[0])
        tail = output[len(source_bytes):]
        if f"/Prev {previous}".encode() not in tail:
            raise AssertionError("wrong Prev address")
        check_pdf(target)
        reader = PdfReader(target, strict=True)
        if reader.metadata.title != f"Incremental {index}" or len(reader.pages) != 2:
            raise AssertionError("independent metadata/page readback failed")
        source, source_bytes = target, output
    if render(source, f"{label}-final") != baseline:
        raise AssertionError("metadata-only save changed rendered pages")
    return {"file": str(source), "sha256": hashlib.sha256(source_bytes).hexdigest()}


versions = {"qpdf": run("qpdf", "--version"), "mutool": run("mutool", "-v")}
versions.update({"pikepdf": pikepdf.__version__, "libqpdf": pikepdf.__libqpdf_version__})
stream_source = EVIDENCE / "object-stream.pdf"
run("qpdf", "--object-streams=generate", FIXTURE, stream_source)
source_reader = PdfReader(stream_source, strict=True)
root = source_reader.trailer.raw_get("/Root")
info = source_reader.trailer.raw_get("/Info")
ids = source_reader.trailer["/ID"]
id_hex = " ".join(f"<{item.original_bytes.hex()}>" for item in ids)
source_bytes = stream_source.read_bytes()
prefix = source_bytes.rsplit(b"startxref", 1)[0]
stream_offset = int(source_bytes.rsplit(b"startxref", 1)[1].split()[0])
hybrid_source = EVIDENCE / "hybrid.pdf"
hybrid_source.write_bytes(prefix + (
    f"xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Root {root.idnum} {root.generation} R /Info {info.idnum} {info.generation} R "
    f"/Size {source_reader.trailer['/Size']} /ID [{id_hex}] /XRefStm {stream_offset} >>\nstartxref\n{len(prefix)}\n%%EOF\n"
).encode("ascii"))
check_pdf(hybrid_source)
sources = [("table", FIXTURE), ("stream", stream_source), ("hybrid", hybrid_source)]
results = {label: verify_chain(source, label) for label, source in sources}

for label, source in sources:
    old_info = PdfReader(source, strict=True).trailer.raw_get("/Info")
    deleted = EVIDENCE / f"{label}-deleted.pdf"
    run(EXAMPLE, source, deleted, "Deletion", f"--delete-object={old_info.idnum},{old_info.generation}")
    check_pdf(deleted, deleted_compressed=(old_info.idnum, old_info.generation) if label != "table" else None)
    if label == "table" and run("qpdf", f"--show-object={old_info.idnum},{old_info.generation}", deleted) != "null":
        raise AssertionError("deleted object still resolves")
    if PdfReader(deleted, strict=True).metadata.title != "Deletion":
        raise AssertionError("metadata replacement after deletion failed")

removed_info = EVIDENCE / "trailer-only.pdf"
run(EXAMPLE, FIXTURE, removed_info, "Unused", "--remove-info")
check_pdf(removed_info)
if PdfReader(removed_info, strict=True).metadata is not None:
    raise AssertionError("removed Info inherited from older revision")

demo_baseline = None
for label, source in sources:
    demo = EVIDENCE / f"interactions-{label}.pdf"
    run(EXAMPLE, source, demo, "Interactions", "--demo-interactions")
    check_pdf(demo)
    reader = PdfReader(demo, strict=True)
    fields = reader.get_fields()
    if fields["external_text"]["/V"] != "Incremental" or fields["external_check"]["/V"] != "/Off":
        raise AssertionError("independent form values mismatch")
    if reader.outline[0].title != "Updated bookmark":
        raise AssertionError("independent bookmark mismatch")
    if reader.attachments["new.txt"] != [b"payload"]:
        raise AssertionError("independent attachment mismatch")
    pixels = render(demo, f"interactions-{label}")
    if demo_baseline is not None and pixels != demo_baseline:
        raise AssertionError("interaction appearance differs across input formats")
    demo_baseline = pixels

for kind, flags in [("linearized", ["--linearize"]), ("encrypted", ["--encrypt", "test-user", "test-owner", "256", "--"])]:
    source = EVIDENCE / f"{kind}.pdf"
    run("qpdf", *flags, FIXTURE, source)
    target = EVIDENCE / f"{kind}-rejected.pdf"
    run(EXAMPLE, source, target, "Rejected", succeeds=False)
    if target.exists():
        raise AssertionError("rejected input created output")

print(json.dumps({"versions": versions, "results": results, "legacy_warnings": LEGACY_WARNINGS, "evidence": str(EVIDENCE), "visual_review": "pending"}, indent=2))

"""Independent synthetic signature gate (no production certificates/keys).

PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-signatures.py
Requires OpenSSL, qpdf, MuPDF, pikepdf and pyHanko 0.37.0. No network trust fetch.
Evidence (including disposable private key) stays in ignored tmp/pdfs only.
"""
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

import pikepdf
from pypdf import PdfReader
from asn1crypto import pem, x509
from pyhanko.pdf_utils.reader import PdfFileReader
from pyhanko.version import __version__ as pyhanko_version
from pyhanko.sign.validation import validate_pdf_signature
from pyhanko_certvalidator import ValidationContext

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = Path(tempfile.mkdtemp(prefix="v05-signatures-", dir=ROOT / "tmp/pdfs"))
EXAMPLE = ROOT / "target/debug/examples/v05_sign"


def run(*args, succeeds=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if (result.returncode == 0) != succeeds:
        raise RuntimeError(f"unexpected exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()


key, cert = EVIDENCE / "synthetic-key.pem", EVIDENCE / "synthetic-cert.pem"
run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", cert,
    "-days", "1", "-subj", "/CN=rzpdf synthetic acceptance only", "-addext", "basicConstraints=critical,CA:TRUE",
    "-addext", "keyUsage=critical,digitalSignature,nonRepudiation,keyCertSign,cRLSign")
key.chmod(0o600)
root_cert = x509.Certificate.load(pem.unarmor(cert.read_bytes())[2])


def validate(path):
    with path.open("rb") as stream:
        reader = PdfFileReader(stream, strict=True)
        assert len(reader.embedded_signatures) == 1
        status = validate_pdf_signature(reader.embedded_signatures[0],
            signer_validation_context=ValidationContext(trust_roots=[root_cert], allow_fetching=False,
                revocation_mode="soft-fail"))
        return {"intact": status.intact, "valid": status.valid, "trusted_test_root_only": status.trusted,
                "coverage": status.coverage.name, "modification_level": status.modification_level.name if status.modification_level else None}


def render(path, name):
    run("mutool", "draw", "-q", "-r", "72", "-o", EVIDENCE / f"{name}-%d.png", path)
    return [p.read_bytes() for p in sorted(EVIDENCE.glob(f"{name}-*.png"))]


source = ROOT / "pdf/tests/fixtures/v04/reportlab.pdf"
source_png = render(source, "source")
results = {}
for mode in ["disable", "generate", "hybrid"]:
    original = EVIDENCE / f"source-{mode}.pdf"
    if mode == "hybrid":
        stream_source = EVIDENCE / "source-generate.pdf"
        reader = PdfReader(stream_source, strict=True)
        root, info = reader.trailer.raw_get("/Root"), reader.trailer.raw_get("/Info")
        ids = " ".join(f"<{item.original_bytes.hex()}>" for item in reader.trailer["/ID"])
        raw = stream_source.read_bytes()
        prefix = raw.rsplit(b"startxref", 1)[0]
        offset = int(raw.rsplit(b"startxref", 1)[1].split()[0])
        original.write_bytes(prefix + (f"xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Root {root.idnum} {root.generation} R /Info {info.idnum} {info.generation} R /Size {reader.trailer['/Size']} /ID [{ids}] /XRefStm {offset} >>\nstartxref\n{len(prefix)}\n%%EOF\n").encode())
        rejected = EVIDENCE / "hybrid-must-not-sign.pdf"
        assert "hybrid" in run(EXAMPLE, original, rejected, cert, key, succeeds=False)
        assert not rejected.exists()
        appended_unsigned = EVIDENCE / "hybrid-history.pdf"
        run(ROOT / "target/debug/examples/v05_incremental", original, appended_unsigned, "hybrid history")
        assert "hybrid" in run(EXAMPLE, appended_unsigned, rejected, cert, key, succeeds=False)
        assert not rejected.exists()
        # Explicit independent full normalization, not a hidden signing fallback.
        normalized = EVIDENCE / "hybrid-normalized.pdf"
        run("qpdf", "--object-streams=disable", original, normalized)
        original = normalized
    else:
        run("qpdf", f"--object-streams={mode}", source, original)
    case = "hybrid-normalized" if mode == "hybrid" else mode
    signed = EVIDENCE / f"signed-{case}.pdf"
    assert "negative completion and retry checks passed" in run(EXAMPLE, original, signed, cert, key, "--self-test")
    assert signed.read_bytes().startswith(original.read_bytes())
    run("qpdf", "--check", signed)
    with pikepdf.Pdf.open(signed, attempt_recovery=False) as pdf:
        assert not pdf.check_pdf_syntax()
        assert len(pdf.pages) == 2
        assert len(pdf.Root.AcroForm.Fields) == 3
    independent = PdfReader(signed, strict=True)
    original_reader = PdfReader(original, strict=True)
    for field in ["external_text", "external_check"]:
        assert independent.get_fields()[field]["/V"] == original_reader.get_fields()[field]["/V"]
    assert independent.outline[0].title == original_reader.outline[0].title
    status = validate(signed)
    assert status["intact"] and status["valid"] and status["trusted_test_root_only"]
    assert status["coverage"] == "ENTIRE_FILE"
    assert render(signed, f"signed-{mode}") == source_png
    inspection = run(EXAMPLE, "--inspect", signed)
    assert "covers_current=true" in inspection and "verification=NotPerformed" in inspection
    results[case] = {"sha256": hashlib.sha256(signed.read_bytes()).hexdigest(), "validation": status, "core_report": inspection}
    if mode == "hybrid":
        results[case]["normalization"] = "explicit qpdf full rewrite; original hybrid and hybrid-history signing rejected"

    # Same-length, syntactically valid header mutation is covered by the hash.
    tampered = EVIDENCE / f"tampered-{mode}.pdf"
    altered = bytearray(signed.read_bytes())
    altered[7] = ord("6") if altered[7] != ord("6") else ord("7")
    tampered.write_bytes(altered)
    assert not validate(tampered)["intact"]

    # Independent append: old signature is mathematically intact but no longer
    # covers the whole file. Core must refuse editing/rewrite regardless.
    with pikepdf.Pdf.open(signed) as pdf:
        size = int(pdf.trailer.Size)
        root_number, root_generation = pdf.Root.objgen
    prefix = signed.read_bytes()
    previous = int(prefix.rsplit(b"startxref", 1)[1].split()[0])
    extra = f"\n{size} 0 obj\n<< /Producer (independent synthetic append) >>\nendobj\n".encode()
    offset = len(prefix) + 1
    xref_offset = len(prefix) + len(extra)
    tail = (f"xref\n{size} 1\n{offset:010} 00000 n \ntrailer\n<< /Size {size + 1} /Root {root_number} {root_generation} R /Info {size} 0 R /Prev {previous} >>\nstartxref\n{xref_offset}\n%%EOF\n").encode()
    appended = EVIDENCE / f"appended-{mode}.pdf"
    appended.write_bytes(prefix + extra + tail)
    run("qpdf", "--check", appended)
    appended_status = validate(appended)
    assert appended_status["intact"] and appended_status["valid"]
    assert appended_status["coverage"] == "ENTIRE_REVISION"
    assert "covers_current=false" in run(EXAMPLE, "--inspect", appended)
    results[case]["append_validation"] = appended_status

report = {"evidence": str(EVIDENCE), "results": results, "visual_review": "pending", "trust": "synthetic explicit local root only; network fetching disabled", "versions": {"qpdf": run("qpdf", "--version"), "openssl": run("openssl", "version"), "mutool": run("mutool", "-v"), "pyHanko": pyhanko_version}}
(EVIDENCE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))

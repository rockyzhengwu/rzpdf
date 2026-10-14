"""Independent R6 checks; requires qpdf, MuPDF, pikepdf and pypdf.

PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v05-security.py
Only synthetic test passwords are used. No production key/password material.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

import pikepdf
from pypdf import PdfReader

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = ROOT / "target/debug/examples/v05_security"
EVIDENCE = Path(tempfile.mkdtemp(prefix="v05-security-", dir=ROOT / "tmp/pdfs"))
FIXTURE = ROOT / "pdf/tests/fixtures/v04/reportlab.pdf"
USER, OWNER = "synthetic-user", "synthetic-owner"


def run(*args, succeeds=True, env=None):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, env=env)
    if (result.returncode == 0) != succeeds:
        raise RuntimeError(f"unexpected exit {result.returncode}: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout.strip() or result.stderr.strip()


def example(source, target, mode, password=None, user=USER, owner=OWNER, flags=(), fill=False, succeeds=True):
    environment = {key: value for key, value in os.environ.items() if not key.startswith("RZPDF_")}
    if password is not None:
        environment["RZPDF_INPUT_PASSWORD"] = password
    environment.update({"RZPDF_OUTPUT_USER_PASSWORD": user, "RZPDF_OUTPUT_OWNER_PASSWORD": owner})
    environment.update({flag: "1" for flag in flags})
    return run(EXAMPLE, source, target, mode, *(["--fill"] if fill else []), succeeds=succeeds, env=environment)


def check(path, password=None):
    with pikepdf.Pdf.open(path, password=password or "", attempt_recovery=False) as reader:
        warnings = reader.check_pdf_syntax()
        if warnings:
            raise AssertionError(warnings)
    run("qpdf", *([f"--password={password}"] if password is not None else []), "--check", path)


def render(path, prefix, password=None):
    run("mutool", "draw", "-q", "-r", "72", *(["-p", password] if password is not None else []), "-o", EVIDENCE / f"{prefix}-%d.png", path)
    return [path.read_bytes() for path in sorted(EVIDENCE.glob(f"{prefix}-*.png"))]


source = EVIDENCE / "source.pdf"
with pikepdf.Pdf.open(FIXTURE) as reader:
    reader.attachments["security.bin"] = bytes(range(34))
    with reader.open_metadata() as metadata:
        metadata["dc:title"] = "R6 interoperability fixture"
    reader.save(source)
check(source)
baseline = render(source, "source")
results = []


def readback(path, password, changed=False):
    with pikepdf.Pdf.open(path, password=password, attempt_recovery=False) as reader:
        if not reader.is_encrypted or reader.encryption.R != 6 or reader.encryption.V != 5:
            raise AssertionError("wrong protection revision")
        if reader.attachments["security.bin"].get_file().read_bytes() != bytes(range(34)):
            raise AssertionError("attachment payload mismatch")
        if not reader.Root.Metadata.read_bytes():
            raise AssertionError("metadata was lost")
    # pypdf crypto is optional on this host; decrypt a temporary independent
    # qpdf output before using its form/tree reader.
    decoded = EVIDENCE / f"{path.stem}-decoded.pdf"
    run("qpdf", f"--password={password}", "--decrypt", path, decoded)
    reader = PdfReader(decoded, strict=True)
    expected = "Protected update" if changed else "External"
    if reader.get_fields()["external_text"]["/V"] != expected:
        raise AssertionError("independent form readback mismatch")
    if len(reader.pages) != 2 or len(reader.outline) != 2:
        raise AssertionError("navigation/pages lost")


for stream_mode in ["disable", "generate"]:
    encrypted = EVIDENCE / f"qpdf-{stream_mode}.pdf"
    run("qpdf", f"--object-streams={stream_mode}", "--encrypt", USER, OWNER, "256", "--", source, encrypted)
    example(encrypted, EVIDENCE / "unused", "inspect", password=USER)
    for role, password in [("user", USER), ("owner", OWNER)]:
        target = EVIDENCE / f"preserved-{stream_mode}-{role}.pdf"
        example(encrypted, target, "preserve", password=password)
        check(target, USER)
        readback(target, USER)
        if render(target, target.stem, USER) != baseline:
            raise AssertionError("preserved protected document changed pixels")
    rejected = EVIDENCE / f"wrong-{stream_mode}.pdf"
    example(encrypted, rejected, "preserve", password="wrong", succeeds=False)
    if rejected.exists():
        raise AssertionError("wrong password wrote output")
    removed = EVIDENCE / f"removed-{stream_mode}.pdf"
    example(encrypted, removed, "remove", password=OWNER)
    check(removed)
    with pikepdf.Pdf.open(removed) as reader:
        if reader.is_encrypted or "/Encrypt" in reader.trailer:
            raise AssertionError("explicit removal did not remove protection")
    forbidden = EVIDENCE / f"forbidden-remove-{stream_mode}.pdf"
    example(encrypted, forbidden, "remove", password=USER, succeeds=False)
    if forbidden.exists():
        raise AssertionError("user removed protection")

for label, user, owner, extra in [
    ("unicode", "用户密码", "所有者密码", []),
    ("clear-metadata", USER, OWNER, ["--cleartext-metadata"]),
    ("empty-user", "", OWNER, []),
]:
    encrypted = EVIDENCE / f"qpdf-{label}.pdf"
    run("qpdf", "--encrypt", user, owner, "256", *extra, "--", source, encrypted)
    for role, password in [("user", user), ("owner", owner)]:
        example(encrypted, EVIDENCE / "unused", "inspect", password=password)
    preserved = EVIDENCE / f"qpdf-{label}-preserved.pdf"
    example(encrypted, preserved, "preserve", password=user)
    check(preserved, owner)
    readback(preserved, owner)
    if render(preserved, preserved.stem, user) != baseline:
        raise AssertionError("foreign special-case preservation changed pixels")

for label, clear, user, owner in [
    ("default", False, USER, OWNER), ("clear-metadata", True, USER, OWNER),
    ("empty-user", False, "", OWNER), ("unicode", False, "用户密码", "所有者密码"),
]:
    target = EVIDENCE / f"created-{label}.pdf"
    example(source, target, "encrypt", user=user, owner=owner, flags=["RZPDF_CLEAR_METADATA"] if clear else [])
    check(target, user)
    readback(target, owner)
    if render(target, target.stem, user) != baseline:
        raise AssertionError("new protection changed pixels")
    with pikepdf.Pdf.open(target, password=user, attempt_recovery=False) as reader:
        if bool(reader.trailer.Encrypt.get("/EncryptMetadata", True)) == clear:
            raise AssertionError("EncryptMetadata policy mismatch")
    results.append({"case": label, "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})

replacement = EVIDENCE / "replaced.pdf"
example(EVIDENCE / "created-default.pdf", replacement, "replace", password=OWNER, user="new-user", owner="new-owner")
check(replacement, "new-user")
run("qpdf", f"--password={USER}", "--check", replacement, succeeds=False)

restricted = EVIDENCE / "restricted.pdf"
example(source, restricted, "encrypt", flags=["RZPDF_RESTRICT_MODIFY"])
filled = EVIDENCE / "filled.pdf"
example(restricted, filled, "preserve", password=USER, fill=True)
check(filled, USER)
readback(filled, USER, changed=True)
render(filled, "filled", USER)

no_fill = EVIDENCE / "no-fill.pdf"
example(source, no_fill, "encrypt", flags=["RZPDF_RESTRICT_FILL"])
forbidden = EVIDENCE / "forbidden-fill.pdf"
example(no_fill, forbidden, "preserve", password=USER, fill=True, succeeds=False)
if forbidden.exists():
    raise AssertionError("form filling policy bypass")

for label, extra in [("R5", ["--force-R5"]), ("R4", ["--use-aes=y"])]:
    target = EVIDENCE / f"unsupported-{label}.pdf"
    run("qpdf", "--encrypt", USER, OWNER, "256" if label == "R5" else "128", *extra, "--", source, target)
    error = example(target, EVIDENCE / "unused", "inspect", password=OWNER, succeeds=False)
    if "UnsupportedSecurity" not in error:
        raise AssertionError("unsupported handler confused with password error")

report = {"evidence": str(EVIDENCE), "results": results, "versions": {
    "qpdf": run("qpdf", "--version"), "libqpdf": pikepdf.__libqpdf_version__, "pikepdf": pikepdf.__version__, "mutool": run("mutool", "-v"),
}, "visual_review": "pending"}
(EVIDENCE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))

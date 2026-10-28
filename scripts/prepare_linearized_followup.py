#!/usr/bin/env python3
"""Generate project-authored linearization regression fixtures into a NEW directory.

Requires existing qpdf and OpenSSL; no downloads. Keys stay in the ignored output
folder and are never needed by the tests. Input is the committed two-page fixture.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import zlib

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
SOURCE = ROOT / "pdf/tests/fixtures/linearized/two-pages.pdf"
source = SOURCE.read_bytes()


def run(*args, data=None):
    return subprocess.run([str(a) for a in args], input=data, capture_output=True, check=True).stdout


# Retain all page-object offsets and the primary hint span. The overflow
# object follows the page objects, changing no page hint offsets. Decode(primary)
# + Decode(overflow) is exactly the original hint payload, including /S offsets.
xref_text = run("qpdf", "--show-xref", SOURCE).decode()
offsets = {int(n): int(offset) for n, offset in re.findall(r"(\d+)/0: uncompressed; offset = (\d+)", xref_text)}
assert set(offsets) == set(range(1, 9))
first_xref = int(re.findall(rb"startxref\s+(\d+)", source)[-1])
main_xref = int(re.search(rb"/Prev\s+(\d+)", source)[1])
hint_offset, hint_length = map(int, re.search(rb"/H\s*\[\s*(\d+)\s+(\d+)\s*\]", source).groups())
hint_source = source[hint_offset:hint_offset + hint_length]
shared_offset = int(re.search(rb"/S\s+(\d+)", hint_source)[1])
raw = zlib.decompress(re.search(rb"stream\n(.*?)\nendstream", hint_source, re.S)[1])
primary = zlib.compress(raw[:20])
overflow = zlib.compress(raw[20:])
p = f"6 0 obj\n<< /Filter /FlateDecode /S {shared_offset} /Length {len(primary)} >>\nstream\n".encode() + primary + b"\nendstream\nendobj\n"
p += b" " * (hint_length - len(p))
assert len(p) == hint_length
ov = f"9 0 obj\n<< /Filter /FlateDecode /Length {len(overflow)} >>\nstream\n".encode() + overflow + b"\nendstream\nendobj\n"
main = main_xref + len(ov)
param = source[offsets[4]:first_xref].rstrip() + b"\n"
param = re.sub(rb"/L\s+\d+", f"/L {len(source) + len(ov)}".encode(), param)
param = re.sub(rb"/H\s*\[[^]]+\]", f"/H [{hint_offset} {hint_length} {main_xref} {len(ov)}]".encode(), param)
original_t = int(re.search(rb"/T\s+(\d+)", param)[1])
param = re.sub(rb"/T\s+\d+", f"/T {original_t + len(ov)}".encode(), param)
param += b" " * (first_xref - offsets[4] - len(param))
assert len(param) == first_xref - offsets[4]
entries = b"".join(f"{o:010} 00000 n \n".encode() for o in [offsets[n] for n in range(4, 9)] + [main_xref])
ident = re.search(rb"/ID (\[.*?\])", source)[1]
xref = b"xref\n4 6\n" + entries + f"trailer<</Root 5 0 R/Size 10/Prev {main}/ID".encode() + ident + b">>\nstartxref\n0\n%%EOF\n"
xref += b" " * (offsets[5] - first_xref - len(xref))
assert len(xref) == offsets[5] - first_xref
(OUT / "two-pages-overflow.pdf").write_bytes(source[:offsets[4]] + param + xref + source[offsets[5]:hint_offset] + p + source[hint_offset + hint_length:main_xref] + ov + source[main_xref:])

run("qpdf", "--linearize", "--encrypt", "synthetic-user", "synthetic-owner", "256", "--", SOURCE, OUT / "two-pages-r6.pdf")

# A real detached CMS signature appended to a formerly linearized source.
# Original bytes survive exactly. /Linearized remains in the signed prefix;
# its offsets are no longer current, as expected after an incremental update.
key, cert = OUT / "synthetic-key.pem", OUT / "synthetic-cert.pem"
run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", cert, "-days", "1", "-subj", "/CN=rzpdf synthetic linearization regression")
result = bytearray(source + b"\n")
entries = []
for number, body in [
    (5, b"<< /Type /Catalog /Pages 3 0 R /AcroForm 10 0 R >>"),
    (7, b"<< /Type /Page /Parent 3 0 R /MediaBox [0 0 200 200] /Resources <<>> /Contents 8 0 R /Annots [11 0 R] >>"),
    (10, b"<< /Fields [11 0 R] /SigFlags 3 >>"),
    (11, b"<< /Type /Annot /Subtype /Widget /FT /Sig /T (SyntheticApproval) /V 12 0 R /P 7 0 R /Rect [0 0 0 0] /F 2 >>"),
    (12, b"<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /ByteRange [" + b" " * 83 + b"] /Contents <" + b"0" * 8192 + b"> >>"),
]:
    entries.append((number, len(result)))
    result += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
address = len(result)
result += b"xref\n"
for number, offset in entries:
    result += f"{number} 1\n{offset:010} 00000 n \n".encode()
result += f"trailer\n<< /Size 13 /Root 5 0 R /Prev {first_xref} >>\nstartxref\n{address}\n%%EOF\n".encode()
start = result.index(b"/Contents <", entries[-1][1]) + len(b"/Contents ")
end = start + 8194
ranges = f"0 {start} {end} {len(result) - end}".encode()
position = result.index(b"/ByteRange [") + len(b"/ByteRange [")
result[position:position + 83] = ranges.ljust(83)
content = bytes(result[:start] + result[end:])
cms = run("openssl", "cms", "-sign", "-binary", "-md", "sha256", "-nosmimecap", "-signer", cert, "-inkey", key, "-outform", "DER", data=content)
assert len(cms) <= 4096
result[start + 1:end - 1] = cms.hex().encode().ljust(8192, b"0")
assert result.startswith(source)
(OUT / "two-pages-signed.pdf").write_bytes(result)
(OUT / "signed-content.bin").write_bytes(content)
(OUT / "signature.der").write_bytes(cms)
run("openssl", "cms", "-verify", "-binary", "-inform", "DER", "-in", OUT / "signature.der", "-content", OUT / "signed-content.bin", "-noverify", "-out", OUT / "verified-content.bin")

records = []
for name, password in [("two-pages-overflow.pdf", None), ("two-pages-r6.pdf", "synthetic-user"), ("two-pages-signed.pdf", None)]:
    args = ["qpdf"] + ([f"--password={password}"] if password else [])
    check = run(*args, "--check", OUT / name).decode().replace(str(OUT) + "/", "")
    (OUT / (name + ".check.log")).write_text(check)
    records.append(dict(file=name, sha256=hashlib.sha256((OUT / name).read_bytes()).hexdigest(), check=check))
(OUT / "manifest.json").write_text(json.dumps(dict(source_sha256=hashlib.sha256(source).hexdigest(), qpdf=run("qpdf", "--version").decode(), openssl=run("openssl", "version").decode(), fixtures=records), indent=2) + "\n")
print("Generated overflow, R6 and signed-prefix fixtures; qpdf and detached CMS verification passed")

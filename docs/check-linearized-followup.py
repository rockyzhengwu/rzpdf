"""Independent follow-up gate after web/tests/linearized-save.mjs.

python3 docs/check-linearized-followup.py BROWSER_OUTPUT NEW_DIRECTORY
Requires existing qpdf, OpenSSL and PyMuPDF. No downloads or private keys.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

import fitz

ROOT = Path(__file__).resolve().parents[1]
BROWSER = Path(sys.argv[1]).resolve()
OUT = Path(sys.argv[2]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
FIXTURES = ROOT / "pdf/tests/fixtures/linearized"


def run(label, *args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=120)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


# Verify the signature in the committed bytes independently. Do not treat an
# embedded self-signed test certificate as proof of identity or certificate trust.
signed = (FIXTURES / "two-pages-signed.pdf").read_bytes()
source = (FIXTURES / "two-pages.pdf").read_bytes()
assert signed.startswith(source)
a, b, c, d = map(int, re.search(rb"/ByteRange\s*\[\s*(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s*\]", signed).groups())
assert a == 0 and c + d == len(signed)
padded = bytes.fromhex(signed[b + 1:c - 1].decode())
length_octets = padded[1] & 127
size = 2 + padded[1] if padded[1] < 128 else 2 + length_octets + int.from_bytes(padded[2:2 + length_octets], "big")
(OUT / "signature.der").write_bytes(padded[:size])
(OUT / "signed-content.bin").write_bytes(signed[:b] + signed[c:])
run("cms", "openssl", "cms", "-verify", "-binary", "-inform", "DER", "-in", OUT / "signature.der", "-content", OUT / "signed-content.bin", "-noverify", "-out", OUT / "verified-content.bin")
assert (OUT / "verified-content.bin").read_bytes() == signed[:b] + signed[c:]

browser_report = json.loads((BROWSER / "report.json").read_text())
assert browser_report["status"] == "passed" and len(browser_report["checks"]) == 10
assert not browser_report["errors"]
records = []
for name in ["two-pages", "two-pages-objects", "two-pages-overflow", "two-pages-r6"]:
    encrypted = name.endswith("-r6")
    for transport in ["sync", "worker"]:
        path = BROWSER / f"{transport}-{name}.pdf"
        passwords = ["synthetic-user", "synthetic-owner"] if encrypted else [None]
        for password in passwords:
            args = ["qpdf"] + ([f"--password={password}"] if password else [])
            result = run(f"{transport}-{name}-{password or 'plain'}", *args, "--check", path)
            assert "File is not linearized" in result
        with fitz.open(path) as doc:
            assert doc.needs_pass == encrypted
            if encrypted:
                assert doc.authenticate("synthetic-owner")
            assert len(doc) == 2
            assert doc[0].rotation == 90
            assert any(a.info.get("content") == "linearized save regression" for a in doc[1].annots())
            check = next(row for row in browser_report["checks"] if row["transport"] == transport and row["name"] == name + ".pdf")
            assert check["edited"] == check["after"]
            records.append(dict(file=path.name, sha256=hashlib.sha256(path.read_bytes()).hexdigest(), protection=encrypted))
    with fitz.open(BROWSER / f"sync-{name}.pdf") as direct, fitz.open(BROWSER / f"worker-{name}.pdf") as worker:
        if encrypted:
            assert direct.authenticate("synthetic-user") and worker.authenticate("synthetic-user")
        for dpi in [72, 144]:
            for index in [0, 1]:
                a = direct[index].get_pixmap(dpi=dpi, alpha=True)
                b = worker[index].get_pixmap(dpi=dpi, alpha=True)
                assert (a.width, a.height, a.samples) == (b.width, b.height, b.samples)

(OUT / "report.json").write_text(json.dumps(dict(
    status="passed", signed_prefix_cms="valid detached signature; certificate trust not checked",
    qpdf=run("qpdf-version", "qpdf", "--version").strip(),
    openssl=run("openssl-version", "openssl", "version").strip(),
    pymupdf=fitz.VersionBind, outputs=records, transport_pixel_pairs=16,
), indent=2) + "\n")
print("PASS: committed CMS verified; 8 browser outputs checked independently, 16 direct/Worker MuPDF pixel pairs")

"""Synthetic offline history/CMS gate. No production keys or network trust.

Build v011_sign_verify first, then run with the existing Python 3.14 environment:
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-signatures.py NEW_DIRECTORY
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pikepdf
from asn1crypto import algos, cms, core as asn1_core, pem, x509
from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
from pyhanko.pdf_utils.reader import PdfFileReader
from pyhanko.sign import signers
from pyhanko.sign.fields import SigFieldSpec, SigSeedSubFilter
from pyhanko.sign.validation import validate_pdf_signature
from pyhanko_certvalidator import ValidationContext
from pyhanko.version import __version__ as pyhanko_version

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXAMPLE = ROOT / "target/debug/examples/v011_sign_verify"
records = []


def run(label, *args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=120)
    (OUT / f"{label}.log").write_text(result.stdout + result.stderr)
    if (result.returncode == 0) != success:
        raise RuntimeError(f"{label}: unexpected exit {result.returncode}; see log")
    return result.stdout


def check(path, expected, count=1):
    text = run(path.stem + "-core", EXAMPLE, "verify", path)
    assert f"count={count}" in text, text
    assert text.count(f"verification={expected}") == count, text
    records.append(dict(file=path.name, expected=expected, count=count,
                        sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    return text


def independent(path, cert, count=1):
    root = x509.Certificate.load(pem.unarmor(cert.read_bytes())[2])
    with path.open("rb") as stream:
        reader = PdfFileReader(stream, strict=True)
        assert len(reader.embedded_signatures) == count
        for signature in reader.embedded_signatures:
            status = validate_pdf_signature(signature, signer_validation_context=ValidationContext(
                trust_roots=[root], allow_fetching=False, revocation_mode="soft-fail"), skip_diff=True)
            assert status.intact and status.valid


def change_cms(path, name, change, expected):
    with path.open("rb") as stream:
        signature = PdfFileReader(stream, strict=True).embedded_signatures[0]
        a, b = int(signature.byte_range[1]), int(signature.byte_range[2])
    raw = path.read_bytes()
    assert raw[a:a + 1] == b"<" and raw[b - 1:b] == b">"
    content = cms.ContentInfo.load(bytes.fromhex(raw[a + 1:b - 1].decode()))
    change(content["content"])
    encoded = content.dump(force=True).hex().upper().encode()
    assert len(encoded) <= b - a - 2
    result = OUT / f"{name}.pdf"
    result.write_bytes(raw[:a + 1] + encoded + b"0" * (b - a - 2 - len(encoded)) + raw[b - 1:])
    check(result, expected)


source = OUT / "source.pdf"
with pikepdf.Pdf.new() as pdf:
    pdf.add_blank_page(page_size=(200, 200))
    pdf.save(source, force_version="2.0")
run("openssl-version", "openssl", "version")
run("qpdf-version", "qpdf", "--version")

for kind in ("rsa", "ec"):
    key, cert = OUT / f"{kind}-key.pem", OUT / f"{kind}-cert.pem"
    key_args = ["-newkey", "rsa:2048"] if kind == "rsa" else ["-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1"]
    run(kind + "-keygen", "openssl", "req", "-x509", *key_args, "-nodes", "-keyout", key,
        "-out", cert, "-days", "1", "-subj", "/CN=rzpdf synthetic v011 only",
        "-addext", "basicConstraints=critical,CA:TRUE",
        "-addext", "keyUsage=critical,digitalSignature,nonRepudiation,keyCertSign,cRLSign")
    key.chmod(0o600)

pss_parameters = algos.RSASSAPSSParams(dict(hash_algorithm=dict(algorithm="sha256"),
    mask_gen_algorithm=dict(algorithm="mgf1", parameters=dict(algorithm="sha256")), salt_length=32))

for algorithm in ("rsa", "pss", "ec"):
    kind = "ec" if algorithm == "ec" else "rsa"
    cert, key = OUT / f"{kind}-cert.pem", OUT / f"{kind}-key.pem"
    core = OUT / f"core-{algorithm}.pdf"
    run(core.stem + "-sign", EXAMPLE, "sign", source, core, cert, key, algorithm, "Approval")
    check(core, "Valid")
    independent(core, cert)
    run(core.stem + "-qpdf", "qpdf", "--check", core)
    with core.open("rb") as stream:
        gap = PdfFileReader(stream, strict=True).embedded_signatures[0].byte_range
        a, b = int(gap[1]), int(gap[2])
    bad_padding = bytearray(core.read_bytes())
    bad_padding[b - 2] = ord("1")
    padded = OUT / f"nonzero-padding-{algorithm}.pdf"
    padded.write_bytes(bad_padding)
    check(padded, "Malformed")
    bad_der = bytearray(core.read_bytes())
    bad_der[a + 2] = ord("1")  # SEQUENCE 30 -> SET 31 inside the excluded hole.
    malformed_der = OUT / f"malformed-der-{algorithm}.pdf"
    malformed_der.write_bytes(bad_der)
    check(malformed_der, "Malformed")
    # A current-object-only scanner loses these signatures. Check the old
    # revision's real CMS, even after the signature dictionary is superseded.
    with core.open("rb") as stream:
        old_signature = PdfFileReader(stream, strict=True).embedded_signatures[0]
        sig_number = old_signature.sig_object.container_ref.idnum
    with pikepdf.open(core) as pdf:
        root_number, root_generation = pdf.Root.objgen
        size = int(pdf.trailer.Size)
    raw = core.read_bytes()
    previous_xref = int(raw.rsplit(b"startxref", 1)[1].split()[0])
    for state in ("Replaced", "Removed"):
        body = f"\n{sig_number} 0 obj\nnull\nendobj\n".encode() if state == "Replaced" else b"\n"
        entry = f"{len(raw) + 1:010} 00000 n" if state == "Replaced" else "0000000000 00001 f"
        xref_address = len(raw) + len(body)
        tail = (f"xref\n{sig_number} 1\n{entry} \ntrailer\n<< /Root {root_number} {root_generation} R /Size {size} /Prev {previous_xref} >>\nstartxref\n{xref_address}\n%%EOF\n").encode()
        historical = OUT / f"historical-{state.lower()}-{algorithm}.pdf"
        historical.write_bytes(raw + body + tail)
        text = check(historical, "Valid")
        assert f"state={state}" in text and "current=false" in text
        run(historical.stem + "-qpdf", "qpdf", "--check", historical)
    mechanism = algos.SignedDigestAlgorithm(dict(algorithm="rsassa_pss", parameters=pss_parameters)) if algorithm == "pss" else None
    signer = signers.SimpleSigner.load(str(key), str(cert), signature_mechanism=mechanism)
    assert signer is not None
    previous = source
    for revision in range(1, 4):
        signed = OUT / f"pyhanko-{algorithm}-{revision}.pdf"
        with previous.open("rb") as stream, signed.open("wb") as output:
            signers.sign_pdf(IncrementalPdfFileWriter(stream),
                signers.PdfSignatureMetadata(field_name=f"Group.Approval{revision}", md_algorithm="sha256",
                    subfilter=SigSeedSubFilter.ADOBE_PKCS7_DETACHED), signer,
                new_field_spec=SigFieldSpec(sig_field_name=f"Group.Approval{revision}"), output=output)
        assert signed.read_bytes().startswith(previous.read_bytes())
        report = check(signed, "Valid", revision)
        assert 'field=Some("Group.Approval1")' in report
        independent(signed, cert, revision)
        run(signed.stem + "-qpdf", "qpdf", "--check", signed)
        previous = signed
    # Covered bytes can remain syntactically valid while invalidating all CMS digests.
    damaged = bytearray(previous.read_bytes())
    assert damaged[9] == ord("%")
    damaged[10] ^= 1  # Binary header comment, not the declared PDF version.
    tampered = OUT / f"tampered-{algorithm}.pdf"
    tampered.write_bytes(damaged)
    check(tampered, "Invalid", 3)

    def wrong_digest(signed):
        for attr in signed["signer_infos"][0]["signed_attrs"]:
            if attr["type"].native == "message_digest":
                attr["values"][0] = b"\x11" * 32
    change_cms(core, f"wrong-digest-{algorithm}", wrong_digest, "Invalid")

    def wrong_signature(signed):
        signer_info = signed["signer_infos"][0]
        value = bytearray(signer_info["signature"].native)
        value[-1] ^= 1
        signer_info["signature"] = bytes(value)
    change_cms(core, f"wrong-signature-{algorithm}", wrong_signature, "Invalid")

    def wrong_certificate(signed):
        signed["signer_infos"][0]["sid"].chosen["serial_number"] = 1
    change_cms(core, f"wrong-certificate-{algorithm}", wrong_certificate, "Malformed")

    if algorithm == "pss":
        def wrong_salt(signed):
            signed["signer_infos"][0]["signature_algorithm"]["parameters"]["salt_length"] = 20
        change_cms(core, "unsupported-pss-salt", wrong_salt, "Unsupported")
        def wrong_mask(signed):
            signed["signer_infos"][0]["signature_algorithm"]["parameters"]["mask_gen_algorithm"]["parameters"] = algos.DigestAlgorithm(dict(algorithm="sha384"))
        change_cms(core, "unsupported-pss-mask", wrong_mask, "Unsupported")
    if algorithm == "ec":
        def illegal_ec_parameters(signed):
            signed["signer_infos"][0]["signature_algorithm"]["parameters"] = asn1_core.Null()
        change_cms(core, "unsupported-ec-parameters", illegal_ec_parameters, "Unsupported")

    if algorithm != "rsa":
        # Keep all byte offsets, but remove the required PDF 2.0 declaration.
        # This is an unsupported algorithm/version profile, not a crypto claim.
        changed = core.read_bytes().replace(b"%PDF-2.0", b"%PDF-1.7", 1).replace(b"/Version /2.0", b"/Version /1.7")
        old_version = OUT / f"unsupported-version-{algorithm}.pdf"
        old_version.write_bytes(changed)
        check(old_version, "Unsupported")

# A mathematically signable weak RSA key is outside the declared verifier profile.
weak_key, weak_cert = OUT / "weak-key.pem", OUT / "weak-cert.pem"
run("weak-keygen", "openssl", "req", "-x509", "-newkey", "rsa:1024", "-nodes", "-keyout", weak_key,
    "-out", weak_cert, "-days", "1", "-subj", "/CN=rzpdf unsupported synthetic RSA only")
weak_key.chmod(0o600)
weak_signer = signers.SimpleSigner.load(str(weak_key), str(weak_cert))
weak_pdf = OUT / "unsupported-rsa1024.pdf"
with source.open("rb") as stream, weak_pdf.open("wb") as output:
    signers.sign_pdf(IncrementalPdfFileWriter(stream),
        signers.PdfSignatureMetadata(field_name="Weak", md_algorithm="sha256",
            subfilter=SigSeedSubFilter.ADOBE_PKCS7_DETACHED), weak_signer,
        new_field_spec=SigFieldSpec(sig_field_name="Weak"), output=output)
check(weak_pdf, "Unsupported")

(OUT / "manifest.json").write_text(json.dumps(dict(cases=records,
    trust="only explicitly supplied synthetic local roots in independent validation; core asserts no trust",
    authorization="not tested by this CMS/history gate; independent validation uses skip_diff=True",
    producers=["rzpdf/OpenSSL", "pyHanko"], versions=dict(pyhanko=pyhanko_version, pikepdf=pikepdf.__version__),
    source_sha256=hashlib.sha256(source.read_bytes()).hexdigest()), indent=2) + "\n")
print(f"PASS {len(records)} CMS/history cases: RSA, PSS, ECDSA, two producers, three historical signatures, covered-byte tampering")

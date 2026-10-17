"""Synthetic V11-07 gate; existing Python 3.14 pikepdf/pyHanko environment.

Build v011_sign_verify, then:
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-multisign.py NEW_DIRECTORY
No production credentials, network trust, installation, Poppler or fuzz.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pikepdf
from asn1crypto import cms, pem, x509
from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
from pyhanko.pdf_utils.reader import PdfFileReader
from pyhanko.sign import signers
from pyhanko.sign.fields import SigFieldSpec, SigSeedSubFilter, FieldMDPSpec, FieldMDPAction, MDPPerm
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


def inspect(path, count):
    text = run(path.stem + "-inspect", EXAMPLE, "verify", path)
    assert f"count={count}\n" in text and text.count("verification=Valid") == count, text
    return text


def validate(path, roots, count):
    with path.open("rb") as stream:
        reader = PdfFileReader(stream, strict=True)
        assert len(reader.embedded_signatures) == count
        for index, signature in enumerate(reader.embedded_signatures):
            status = validate_pdf_signature(signature,
                signer_validation_context=ValidationContext(trust_roots=roots, allow_fetching=False,
                    revocation_mode="soft-fail"))
            assert status.intact and status.valid
            # Ordinary approvals use the independent diff policy too: every
            # appended revision must be no worse than form/signature filling.
            assert status.modification_level is None or status.modification_level.name in ("NONE", "LTA_UPDATES", "FORM_FILLING"), status
            assert status.coverage.name == ("ENTIRE_FILE" if index == count - 1 else "ENTIRE_REVISION")


def objects(pdf):
    return {obj.objgen: (obj.unparse(resolved=True), obj.read_raw_bytes() if isinstance(obj, pikepdf.Stream) else None)
            for obj in pdf.objects if obj.is_indirect}


def projection(before, after):
    assert after.read_bytes().startswith(before.read_bytes())
    with pikepdf.open(before, attempt_recovery=False) as old, pikepdf.open(after, attempt_recovery=False) as new:
        assert not new.check_pdf_syntax()
        assert old.Root.unparse(resolved=True) == new.Root.unparse(resolved=True)
        form_id, page_id = old.Root.AcroForm.objgen, old.pages[0].obj.objgen
        a, b = objects(old), objects(new)
        added = set(b) - set(a)
        assert len(added) == 3
        changed = {key for key in a if a[key] != b[key]}
        assert changed == {form_id, page_id}, changed
        old_fields, new_fields = list(old.Root.AcroForm.Fields), list(new.Root.AcroForm.Fields)
        assert [f.objgen for f in new_fields[:-1]] == [f.objgen for f in old_fields]
        widget = new_fields[-1]
        assert widget.objgen in added and widget.V.objgen in added and widget.AP.N.objgen in added
        assert list(widget.Rect) == [0, 0, 0, 0] and widget.P.objgen == page_id
        assert widget.AP.N.read_bytes() == b""
        for key in old.Root.AcroForm.keys():
            if key not in ("/Fields", "/SigFlags"):
                assert old.Root.AcroForm[key].unparse() == new.Root.AcroForm[key].unparse()
        assert int(new.Root.AcroForm.SigFlags) == 3
        for key in old.pages[0].obj.keys():
            if key != "/Annots":
                assert old.pages[0].obj[key].unparse() == new.pages[0].obj[key].unparse()
        assert [a.objgen for a in list(new.pages[0].obj.Annots)[:-1]] == [a.objgen for a in old.pages[0].obj.get("/Annots", [])]
        assert new.pages[0].obj.Annots[-1].objgen == widget.objgen
    with before.open("rb") as a, after.open("rb") as b:
        old_sigs, new_sigs = PdfFileReader(a).embedded_signatures, PdfFileReader(b).embedded_signatures
        for old_sig, new_sig in zip(old_sigs, new_sigs):
            assert list(old_sig.byte_range) == list(new_sig.byte_range)
            assert old_sig.pkcs7_content == new_sig.pkcs7_content


source = OUT / "source.pdf"
with pikepdf.Pdf.new() as pdf:
    for _ in range(2):
        page = pdf.add_blank_page(page_size=(200, 200))
        page.obj.Contents = pdf.make_stream(b"0 0 20 20 re f\n")
    pdf.docinfo.Title = "synthetic V11-07 only"
    pdf.save(source, force_version="2.0")
stream_source = OUT / "source-streams.pdf"
run("source-streams", "qpdf", "--object-streams=generate", source, stream_source)
roots, credentials = [], {}
for kind in ("rsa", "ec"):
    key, cert = OUT / f"{kind}-key.pem", OUT / f"{kind}-cert.pem"
    key_args = ["-newkey", "rsa:2048"] if kind == "rsa" else ["-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1"]
    run(kind + "-keygen", "openssl", "req", "-x509", *key_args, "-nodes", "-keyout", key, "-out", cert,
        "-days", "1", "-subj", "/CN=rzpdf V11-07 synthetic test only", "-addext", "basicConstraints=critical,CA:TRUE",
        "-addext", "keyUsage=critical,digitalSignature,nonRepudiation,keyCertSign,cRLSign")
    key.chmod(0o600)
    roots.append(x509.Certificate.load(pem.unarmor(cert.read_bytes())[2]))
    credentials[kind] = key, cert


def py_seed(path, src=source, kind="rsa", certify=None, field_mdp=None, field_name="Seed"):
    key, cert = credentials[kind]
    signer = signers.SimpleSigner.load(str(key), str(cert))
    with src.open("rb") as stream, path.open("wb") as output:
        signers.sign_pdf(IncrementalPdfFileWriter(stream),
            signers.PdfSignatureMetadata(field_name=field_name, md_algorithm="sha256",
                subfilter=SigSeedSubFilter.ADOBE_PKCS7_DETACHED,
                certify=certify is not None, docmdp_permissions=certify or MDPPerm.FILL_FORMS), signer,
            new_field_spec=SigFieldSpec(sig_field_name=field_name, on_page=1, field_mdp_spec=field_mdp),
            bytes_reserved=4096, output=output)


for layout in ("classic", "streams"):
    for producer in ("core", "pyhanko"):
        initial = OUT / f"{layout}-{producer}-1.pdf"
        src = source if layout == "classic" else stream_source
        if producer == "core":
            key, cert = credentials["rsa"]
            run(initial.stem + "-sign", EXAMPLE, "sign", src, initial, cert, key, "rsa", "Seed")
        else:
            py_seed(initial, src)
        inspect(initial, 1)
        validate(initial, roots, 1)
        previous = initial
        for count, algorithm in ((2, "pss"), (3, "ec"), (4, "rsa")):
            kind = "ec" if algorithm == "ec" else "rsa"
            key, cert = credentials[kind]
            signed = OUT / f"{layout}-{producer}-{count}.pdf"
            run(signed.stem + "-append", EXAMPLE, "append", previous, signed, cert, key, algorithm, f"Approval{count}")
            inspect(signed, count)
            projection(previous, signed)
            validate(signed, roots, count)
            run(signed.stem + "-qpdf", "qpdf", "--check", signed)
            records.append(dict(case=signed.name, signatures=count, algorithm=algorithm,
                source_sha256=hashlib.sha256(previous.read_bytes()).hexdigest(), sha256=hashlib.sha256(signed.read_bytes()).hexdigest()))
            previous = signed

# Public signed fixture plus one deterministic external CMS capsule for Cargo
# tests. Never commit the disposable private key. No trust/date assumptions.
fixture = OUT / "fixture-seed.pdf"
py_seed(fixture)
key, cert = credentials["rsa"]
fixture_second = OUT / "fixture-second.pdf"
run("fixture-second", EXAMPLE, "append", fixture, fixture_second, cert, key, "rsa", "Second")
with fixture_second.open("rb") as stream:
    signature = PdfFileReader(stream).embedded_signatures[-1]
    (OUT / "fixture-second.der").write_bytes(cms.ContentInfo.load(signature.pkcs7_content).dump())

key, cert = credentials["rsa"]
for permission in MDPPerm:
    locked = OUT / f"docmdp-{permission.value}.pdf"
    py_seed(locked, certify=permission)
    inspect(locked, 1)
    refused = OUT / f"refused-docmdp-{permission.value}.pdf"
    run(refused.stem, EXAMPLE, "append", locked, refused, cert, key, "rsa", "Next", success=False)
    assert not refused.exists()
    records.append(dict(case=locked.name, refused=True, sha256=hashlib.sha256(locked.read_bytes()).hexdigest()))
for action in FieldMDPAction:
    locked = OUT / f"fieldmdp-{action.name}.pdf"
    py_seed(locked, field_mdp=FieldMDPSpec(action, None if action == FieldMDPAction.ALL else ["Seed"]))
    inspect(locked, 1)
    refused = OUT / f"refused-fieldmdp-{action.name}.pdf"
    run(refused.stem, EXAMPLE, "append", locked, refused, cert, key, "rsa", "Next", success=False)
    assert not refused.exists()
    records.append(dict(case=locked.name, refused=True, sha256=hashlib.sha256(locked.read_bytes()).hexdigest()))

# Valid CMS is not sufficient when ownership is ambiguous.
for problem in ("shared-form", "shared-widget", "foreign-page"):
    malformed_source = OUT / f"{problem}-source.pdf"
    with pikepdf.open(source) as pdf:
        form = pdf.make_indirect(pikepdf.Dictionary(Fields=pikepdf.Array(), SigFlags=0))
        pdf.Root.AcroForm = form
        if problem == "shared-form":
            pdf.Root.CustomSharedForm = form
        else:
            widget = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Annot,
                Subtype=pikepdf.Name.Widget, FT=pikepdf.Name.Tx, T=pikepdf.String("Existing"),
                Rect=pikepdf.Array([10, 10, 80, 30]), P=pdf.pages[0].obj))
            form.Fields.append(widget)
            pdf.pages[0].obj.Annots = pikepdf.Array([widget])
            if problem == "shared-widget":
                pdf.pages[1].obj.Annots = pikepdf.Array([widget])
            else:
                widget.P = pdf.pages[1].obj
        pdf.save(malformed_source, force_version="2.0")
    signed = OUT / f"{problem}-signed.pdf"
    py_seed(signed, malformed_source)
    inspect(signed, 1)
    refused = OUT / f"refused-{problem}.pdf"
    run(refused.stem, EXAMPLE, "append", signed, refused, cert, key, "rsa", "Next", success=False)
    assert not refused.exists()
    records.append(dict(case=signed.name, refused=True, sha256=hashlib.sha256(signed.read_bytes()).hexdigest()))

# A new file must not replace an existing output, even after successful signing.
existing = OUT / "already-exists.pdf"
existing.write_bytes(b"keep existing synthetic target")
run("no-overwrite", EXAMPLE, "append", fixture, existing, cert, key, "rsa", "Second", success=False)
assert existing.read_bytes() == b"keep existing synthetic target"

(OUT / "manifest.json").write_text(json.dumps(dict(cases=records,
    versions=dict(pikepdf=pikepdf.__version__, pyhanko=pyhanko_version, qpdf=run("qpdf-version", "qpdf", "--version"),
                  openssl=run("openssl-version", "openssl", "version")),
    authorization="approval-only exact object projection and independent pyHanko diff policy; all MDP/field locks refused",
    trust="synthetic explicit local roots only; network disabled; core never asserts trust",
    fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
    fixture_cms_sha256=hashlib.sha256((OUT / "fixture-second.der").read_bytes()).hexdigest()), indent=2) + "\n")
print(f"PASS {len(records)} multisign/constraint cases, four-signature chains, exact object projections and independent diff validation")

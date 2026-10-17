"""Offline v0.11 field/MDP workflow gate. Build v011_field_source,
v011_sign_verify and render_annotations first. Existing environments only:
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v011-workflows.py NEW_DIRECTORY
Synthetic keys never enter PDF core or the repository. No fetching/install/fuzz.
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

import pikepdf
from asn1crypto import cms, pem, x509
from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
from pyhanko.pdf_utils.reader import PdfFileReader
from pyhanko.sign import signers
from pyhanko.sign.fields import FieldMDPAction, FieldMDPSpec, MDPPerm, SigSeedSubFilter
from pyhanko.sign.validation import validate_pdf_signature
from pyhanko_certvalidator import ValidationContext
from pyhanko.version import __version__ as pyhanko_version

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v011_sign_verify"
SOURCE_EXE = ROOT / "target/debug/examples/v011_field_source"
records = []


def run(label, *args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / f"{label}.log").write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout


key, cert = OUT / "synthetic-key.pem", OUT / "synthetic-cert.pem"
run("keygen", "openssl", "req", "-newkey", "rsa:2048", "-x509", "-nodes", "-keyout", key,
    "-out", cert, "-days", "1", "-subj", "/CN=rzpdf synthetic workflow only",
    "-addext", "basicConstraints=critical,CA:TRUE",
    "-addext", "keyUsage=critical,digitalSignature,nonRepudiation,keyCertSign,cRLSign")
key.chmod(0o600)
roots = [x509.Certificate.load(pem.unarmor(cert.read_bytes())[2])]
source = OUT / "source.pdf"
font = re.search(r"font=(\d+)", run("source", SOURCE_EXE, source)).group(1)
streams = OUT / "source-streams.pdf"
run("streams", "qpdf", "--object-streams=generate", source, streams)


def verify(path, count):
    text = run(path.stem + "-verify", EXE, "verify", path)
    assert f"count={count}\n" in text and text.count("verification=Valid") == count, text


def independent(path, count, annotation_source=None):
    with path.open("rb") as stream:
        reader = PdfFileReader(stream, strict=True)
        assert len(reader.embedded_signatures) == count
        for signature in reader.embedded_signatures:
            status = validate_pdf_signature(signature, skip_diff=annotation_source is not None, signer_validation_context=ValidationContext(
                trust_roots=roots, allow_fetching=False, revocation_mode="soft-fail"))
            assert status.intact and status.valid
            if annotation_source is None:
                assert status.docmdp_ok
    if annotation_source is not None:
        # pyHanko 0.37 explicitly does not implement P=3 annotation diff policy.
        # Check the exact supported square-annotation projection independently.
        assert path.read_bytes().startswith(annotation_source.read_bytes())
        with pikepdf.open(annotation_source) as a, pikepdf.open(path) as b:
            assert int(a.Root.Perms.DocMDP.Reference[0].TransformParams.P) == 3
            old = {o.objgen: (o.unparse(resolved=True), o.read_raw_bytes() if isinstance(o,pikepdf.Stream) else None) for o in a.objects if o.is_indirect}
            new = {o.objgen: (o.unparse(resolved=True), o.read_raw_bytes() if isinstance(o,pikepdf.Stream) else None) for o in b.objects if o.is_indirect}
            assert set(old) <= set(new)
            assert {i for i in old if old[i] != new[i]} == {a.pages[0].obj.objgen}
            assert len(set(new)-set(old)) == 2
            page_a, page_b = pikepdf.Dictionary(a.pages[0].obj), pikepdf.Dictionary(b.pages[0].obj)
            old_annots, new_annots = list(page_a.Annots), list(page_b.Annots)
            del page_a.Annots; del page_b.Annots
            assert page_a.unparse() == page_b.unparse()
            assert [i.objgen for i in old_annots] == [i.objgen for i in new_annots[:-1]]
            square = new_annots[-1]
            assert square.Subtype == pikepdf.Name.Square and square.P.objgen == b.pages[0].obj.objgen
            assert list(square.Rect) == [20,20,80,80] and str(square.Contents) == "Review note"
            assert {square.objgen,square.AP.N.objgen} == set(new)-set(old)
            assert square.AP.N.read_bytes() == b"q 1 0 0 RG 1 w .5 .5 59 59 re S Q"
            assert list(square.AP.N.BBox) == [0,0,60,60]
            assert list(square.AP.N.Matrix) == [1,0,0,1,0,0] and not square.AP.N.Resources.keys()
        run(path.stem + "-qpdf", "qpdf", "--check", path)


def seed(path, src=source, permission=None, lock=None):
    signer = signers.SimpleSigner.load(str(key), str(cert))
    # pyHanko's signer only accepts one annotation per field. Core multi-Widget
    # signing is independently checked above; constrain this producer's seed.
    locked_source = OUT / (path.stem + "-unsigned.pdf")
    with pikepdf.open(src) as pdf:
        target = pdf.Root.AcroForm.Fields[1].Kids[0]
        # Locked prevents moving/deleting an annotation, not filling its field.
        pdf.Root.AcroForm.Fields[1].Kids[1].F = 132
        removed = target.Kids[1].objgen
        target.Kids = pikepdf.Array([target.Kids[0]])
        pdf.pages[1].obj.Annots = pikepdf.Array([a for a in pdf.pages[1].obj.Annots if a.objgen != removed])
        if lock is not None:
            target.Lock = pikepdf.Dictionary(Type=pikepdf.Name.SigFieldLock,
                Action=pikepdf.Name(lock.action.value))
            if lock.action != FieldMDPAction.ALL:
                target.Lock.Fields = pikepdf.Array([pikepdf.String(n) for n in lock.fields])
        pdf.save(locked_source, force_version="2.0")
    src = locked_source
    with src.open("rb") as inp, path.open("wb") as output:
        signers.sign_pdf(IncrementalPdfFileWriter(inp), signers.PdfSignatureMetadata(
            field_name="Sign.First", md_algorithm="sha256", subfilter=SigSeedSubFilter.ADOBE_PKCS7_DETACHED,
            certify=permission is not None, docmdp_permissions=permission or MDPPerm.FILL_FORMS),
            signer, existing_fields_only=True, bytes_reserved=8192, output=output)
    verify(path, 1)


def projection(before, after, field_name):
    assert after.read_bytes().startswith(before.read_bytes())
    with pikepdf.open(before) as a, pikepdf.open(after) as b:
        old = {o.objgen: (o.unparse(resolved=True), o.read_raw_bytes() if isinstance(o, pikepdf.Stream) else None) for o in a.objects if o.is_indirect}
        new = {o.objgen: (o.unparse(resolved=True), o.read_raw_bytes() if isinstance(o, pikepdf.Stream) else None) for o in b.objects if o.is_indirect}
        # Page content/resources/geometry, catalog, structural and OCG relations stay identical.
        assert a.Root.unparse(resolved=True) == b.Root.unparse(resolved=True)
        assert [p.obj.unparse(resolved=True) for p in a.pages] == [p.obj.unparse(resolved=True) for p in b.pages]
        def fields(pdf):
            result = {}
            def visit(field, prefix=""):
                name = prefix + ("." if prefix else "") + str(field.get("/T", ""))
                if "/T" in field:
                    result[name] = field
                for kid in field.get("/Kids", []):
                    if "/T" in kid:
                        visit(kid, name)
            for field in pdf.Root.AcroForm.Fields:
                visit(field)
            return result
        x, y = fields(a), fields(b)
        assert x.keys() == y.keys()
        allowed = {x[field_name].objgen, a.Root.AcroForm.objgen}
        allowed.update(k.objgen for k in x[field_name].get("/Kids", []))
        assert {obj for obj in old if old[obj] != new[obj]} <= allowed
        for name in x:
            if name != field_name:
                assert x[name].unparse(resolved=True) == y[name].unparse(resolved=True)


def approve(before, after, field="Sign.Second", text="Approval only", font_id=None):
    args = [EXE, "field", before, after, cert, key, "rsa", field, text]
    if font_id is not None:
        args.append(font_id)
    run(after.stem, *args)
    projection(before, after, field)


# Core-produced hierarchical two-Widget Unicode first signature, then another
# signature after an unsigned-but-authorized form-fill and checkbox revision.
for label, src in (("classic", source), ("streams", streams)):
    first = OUT / f"{label}-first.pdf"
    # qpdf may renumber objects: discover the embedded Type0 font by type.
    with pikepdf.open(src) as pdf:
        font_id = str(next(o.objgen[0] for o in pdf.objects if isinstance(o, pikepdf.Dictionary) and o.get("/Subtype") == pikepdf.Name.Type0))
    approve(src, first, "Sign.First", "你好 Hello\nPDF", font_id)
    verify(first, 1)
    independent(first, 1)
    filled = OUT / f"{label}-filled.pdf"
    run(filled.stem, EXE, "fill", first, filled, "Data.Editable", "Hello PDF")
    projection(first, filled, "Data.Editable")
    checked = OUT / f"{label}-checked.pdf"
    run(checked.stem, EXE, "check", filled, checked, "Data.Check", "on")
    projection(filled, checked, "Data.Check")
    second = OUT / f"{label}-second.pdf"
    approve(checked, second)
    verify(second, 2)
    independent(second, 2)
    assert "Unsupported" not in run(second.stem + "-audit", EXE, "audit", second)
    records.append(dict(case=second.name, signatures=2, unsigned_authorized_tail=True))


for permission in MDPPerm:
    signed = OUT / f"docmdp-{permission.value}.pdf"
    seed(signed, permission=permission)
    audit = run(signed.stem + "-audit", EXE, "audit", signed)
    assert ["NoChanges", "FillFormsAndSign", "Annotate"][permission.value-1] in audit
    for operation, args, allowed in (
        ("fill", ["Data.Editable", "Changed"], permission.value >= 2),
        ("field", [cert, key, "rsa", "Sign.Second", "Approval only"], permission.value >= 2),
        ("square", ["0", "Review note"], permission.value == 3)):
        result = OUT / f"docmdp-{permission.value}-{operation}.pdf"
        run(result.stem, EXE, operation, signed, result, *args, success=allowed)
        assert result.exists() == allowed
        if allowed:
            verify(result, 2 if operation == "field" else 1)
            independent(result, 2 if operation == "field" else 1, signed if operation == "square" else None)
            assert "Forbidden" not in run(result.stem + "-audit", EXE, "audit", result)
        records.append(dict(case=result.name, allowed=allowed))

for action, names, target, allowed in (
    (FieldMDPAction.ALL, None, "Data.Editable", False),
    (FieldMDPAction.INCLUDE, ["Data"], "Data.Editable", False),
    (FieldMDPAction.INCLUDE, ["Sign.Second"], "Data.Editable", True),
    (FieldMDPAction.EXCLUDE, ["Data"], "Data.Editable", True),
    (FieldMDPAction.EXCLUDE, ["Sign.Second"], "Data.Editable", False)):
    label = f"lock-{action.name}-{len(records)}"
    signed = OUT / f"{label}.pdf"
    seed(signed, lock=FieldMDPSpec(action, names))
    run(label + "-audit", EXE, "audit", signed)
    output = OUT / f"{label}-filled.pdf"
    run(output.stem, EXE, "fill", signed, output, target, "Allowed value", success=allowed)
    assert output.exists() == allowed
    if allowed:
        verify(output, 1)
        independent(output, 1)
    records.append(dict(case=label, allowed=allowed))


def append_object(src, output, object_id, body):
    original = src.read_bytes()
    previous = int(re.findall(rb"startxref\s+(\d+)", original)[-1])
    with pikepdf.open(src) as pdf:
        root = pdf.Root.objgen
        size = int(pdf.trailer.Size)
    data = original + b"\n"
    offset = len(data)
    data += f"{object_id[0]} {object_id[1]} obj\n".encode() + body + b"\nendobj\n"
    xref = len(data)
    data += f"xref\n{object_id[0]} 1\n{offset:010} {object_id[1]:05} n \ntrailer\n<< /Size {size} /Root {root[0]} {root[1]} R /Prev {previous} >>\nstartxref\n{xref}\n%%EOF\n".encode()
    output.write_bytes(data)


base = OUT / "docmdp-2.pdf"
for problem in ("page-geometry", "shared-font", "structure", "optional-content", "rename", "remove-perms"):
    with pikepdf.open(base) as pdf:
        if problem == "page-geometry":
            obj = pdf.pages[0].obj
            obj.CropBox = pikepdf.Array([0, 0, 500, 500])
        elif problem == "shared-font":
            obj = next(o for o in pdf.objects if isinstance(o, pikepdf.Dictionary) and o.get("/Subtype") == pikepdf.Name.Type0)
            obj.BaseFont = pikepdf.Name.ChangedSharedFont
        elif problem == "rename":
            obj = pdf.Root.AcroForm.Fields[0]
            obj.T = pikepdf.String("Renamed")
        else:
            obj = pdf.Root
            if problem == "structure":
                obj.StructTreeRoot = pikepdf.Dictionary(Type=pikepdf.Name.StructTreeRoot)
            elif problem == "optional-content":
                obj.OCProperties = pikepdf.Dictionary(OCGs=pikepdf.Array(), D=pikepdf.Dictionary())
            else:
                del obj.Perms
        output = OUT / f"forbidden-{problem}.pdf"
        append_object(base, output, obj.objgen, obj.unparse(resolved=True))
    verify(output, 1)  # old signature bits are still valid; authorization is not
    audit = run(output.stem + "-audit", EXE, "audit", output)
    assert "Unsupported" in audit or "Forbidden" in audit
    refused = OUT / f"refused-{problem}.pdf"
    run(refused.stem, EXE, "field", output, refused, cert, key, "rsa", "Sign.Second", success=False)
    assert not refused.exists()
    records.append(dict(case=problem, cryptography_valid=True, update_refused=True))


# Compact reusable public fixture and deterministic CMS capsule. No keys committed.
with (OUT / "docmdp-2-field.pdf").open("rb") as stream:
    (OUT / "fixture-second.der").write_bytes(cms.ContentInfo.load(PdfFileReader(stream).embedded_signatures[-1].pkcs7_content).dump())


def altered_constraint(original, output, old, new):
    # Equal-width replacement in the signature dictionary followed by a fresh
    # external CMS. Thus a declaration can be invalid while the signature bits
    # and ByteRange are genuinely valid (not merely a tampered-signature test).
    with original.open("rb") as stream:
        signature = PdfFileReader(stream).embedded_signatures[0]
        ranges = list(signature.byte_range)
        number = signature.sig_object.container_ref.idnum
    data = bytearray(original.read_bytes())
    start = data.rfind(f"\n{number} 0 obj".encode())
    assert start >= 0 and len(old) == len(new)
    at = data.find(old, start)
    assert at >= start
    data[at:at+len(old)] = new
    content = bytes(data[:ranges[1]] + data[ranges[2]:ranges[2]+ranges[3]])
    result = subprocess.run(["openssl", "cms", "-sign", "-binary", "-md", "sha256", "-signer", str(cert),
        "-inkey", str(key), "-outform", "DER"], input=content, capture_output=True, timeout=60, check=True)
    encoded = result.stdout.hex().encode().upper()
    capacity = ranges[2]-ranges[1]-2
    assert len(encoded) <= capacity
    data[ranges[1]+1:ranges[2]-1] = encoded + b"0"*(capacity-len(encoded))
    output.write_bytes(data)
    verify(output, 1)


for label, old, new in (("invalid-p0", b"/P 2", b"/P 0"), ("invalid-p4", b"/P 2", b"/P 4"),
    ("unknown-transform", b"/TransformMethod /DocMDP", b"/TransformMethod /BadMDP"),
    ("unknown-version", b"/V /1.2", b"/V /2.0")):
    invalid = OUT / f"{label}.pdf"
    altered_constraint(base, invalid, old, new)
    run(label + "-audit", EXE, "audit", invalid, success=False)
    refused = OUT / f"refused-{label}.pdf"
    run(refused.stem, EXE, "field", invalid, refused, cert, key, "rsa", "Sign.Second", success=False)
    assert not refused.exists()
    records.append(dict(case=label, cryptography_valid=True, unsupported_constraints_refused=True))

default_permission = OUT / "default-p.pdf"
altered_constraint(base, default_permission, b"/P 2", b"    ")
assert "FillFormsAndSign" in run("default-p-audit", EXE, "audit", default_permission)
default_result = OUT / "default-p-signed.pdf"
approve(default_permission, default_result)
verify(default_result, 2)
# pyHanko 0.37 requires /P despite the standard's default. Do not patch its
# parser or rewrite signed bytes: use OpenSSL CMS verification plus the exact
# field projection above and an independent direct check of the default.
with pikepdf.open(default_permission) as pdf:
    assert "/P" not in pdf.Root.Perms.DocMDP.Reference[0].TransformParams
with default_result.open("rb") as stream:
    for index, signature in enumerate(PdfFileReader(stream).embedded_signatures):
        r = list(signature.byte_range)
        data = default_result.read_bytes()
        content = OUT / f"default-p-{index}-content.bin"
        der = OUT / f"default-p-{index}-cms.der"
        content.write_bytes(data[:r[1]] + data[r[2]:r[2]+r[3]])
        der.write_bytes(cms.ContentInfo.load(signature.pkcs7_content).dump())
        run(f"default-p-{index}-openssl", "openssl", "cms", "-verify", "-binary", "-inform", "DER", "-noverify",
            "-in", der, "-content", content, "-out", "/dev/null")
records.append(dict(case="default-p", allowed=True))

existing = OUT / "already-exists.pdf"
existing.write_bytes(b"keep synthetic existing target")
for operation,args in (("fill",["Data.Editable","Allowed"]),("field",[cert,key,"rsa","Sign.Second"])):
    run("no-overwrite-"+operation,EXE,operation,base,existing,*args,success=False)
    assert existing.read_bytes() == b"keep synthetic existing target"

# Independent page-level artwork, not a copy of the core AP stream. Compare
# actual annotation placement with plain page painting in each renderer.
from PIL import Image, ImageChops
expected_first = OUT / "render-reference-first.pdf"
with pikepdf.open(source) as pdf:
    # The actual field is filled, so MuPDF no longer draws its orange unsigned
    # SIGN prompt. Remove that empty field only in the render-only reference;
    # its expected appearance is painted independently as page content below.
    group = pdf.Root.AcroForm.Fields[1]
    first_field = group.Kids[0]
    first_widgets = {w.objgen for w in first_field.Kids}
    group.Kids = pikepdf.Array(list(group.Kids)[1:])
    for page in pdf.pages:
        page.obj.Annots = pikepdf.Array([a for a in page.obj.Annots if a.objgen not in first_widgets])
    noto = next(o for o in pdf.objects if isinstance(o,pikepdf.Dictionary) and o.get("/Subtype") == pikepdf.Name.Type0)
    cmap = noto.ToUnicode.read_bytes().decode("ascii").split("beginbfchar",1)[1].split("endbfchar",1)[0]
    code = {bytes.fromhex(dst).decode("utf-16-be"): src for src,dst in re.findall(r"<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>",cmap)}
    for page in pdf.pages:
        resources = pikepdf.Dictionary(page.obj.Resources)
        fonts = pikepdf.Dictionary(resources.get("/Font",pikepdf.Dictionary()))
        fonts.ExpectedNoto = noto
        resources.Font = fonts
        page.obj.Resources = resources
        content = "q 1 0 0 1 20 180 cm 1 1 1 rg 0 0 280 50 re f 0 G 1 w .5 .5 279 49 re S 2 2 276 46 re W n\n"
        for text,y in (("你好 Hello",37.4),("PDF",23.0)):
            encoded = "".join(code[ch] for ch in text)
            content += f"0 g BT /ExpectedNoto 12 Tf 1 0 0 1 3 {y:g} Tm <{encoded}> Tj ET\n"
        content += "Q\n"
        old = page.obj.Contents
        page.obj.Contents = pikepdf.Array([*list(old),pdf.make_stream(content.encode())] if isinstance(old,pikepdf.Array) else [old,pdf.make_stream(content.encode())])
    pdf.save(expected_first, force_version="2.0")
expected_square = OUT / "render-reference-square.pdf"
with pikepdf.open(OUT / "docmdp-3.pdf") as pdf:
    page = pdf.pages[0]
    old = page.obj.Contents
    art = pdf.make_stream(b"q 1 0 0 1 20 20 cm 1 0 0 RG 1 w .5 .5 59 59 re S Q\n")
    page.obj.Contents = pikepdf.Array([*list(old),art] if isinstance(old,pikepdf.Array) else [old,art])
    pdf.save(expected_square, force_version="2.0")

visual = []
for label,actual,expected in (("unicode",OUT/"classic-first.pdf",expected_first),("square",OUT/"docmdp-3-square.pdf",expected_square)):
    for engine in ("mupdf","skia"):
        for kind,path in (("actual",actual),("expected",expected)):
            prefix = OUT / f"{label}-{engine}-{kind}"
            if engine == "mupdf":
                prefix.mkdir()
                run(prefix.name,"mutool","draw","-r","144","-o",prefix/"page-%d.png",path)
            else:
                run(prefix.name,ROOT/"target/debug/examples/render_annotations",path,prefix,"screen","2")
        for page in (1,2):
            a = Image.open(OUT/f"{label}-{engine}-actual"/f"page-{page}.png").convert("RGB")
            b = Image.open(OUT/f"{label}-{engine}-expected"/f"page-{page}.png").convert("RGB")
            diff = ImageChops.difference(a,b)
            assert a.size == b.size and diff.getbbox() is None,(label,engine,page,diff.getbbox())
            visual.append(dict(case=label,engine=engine,page=page,exact_pixels=True))
# The within-engine references intentionally retain the independent producer's
# original AP. Also compare its text bounds across engines: otherwise both
# actual and reference can share a font-advance bug and still match exactly.
producer_bounds = {}
for engine in ("mupdf", "skia"):
    image = Image.open(OUT/f"square-{engine}-actual"/"page-1.png").convert("L")
    # Synthetic source uses a 612x792 pt page and Rect [20,180,300,230].
    # Exclude the frame and orange unsigned-widget UI from this text-only ROI.
    assert image.size == (1224, 1584)
    region = image.crop((55, 1134, 585, 1214))
    bounds = region.point(lambda value: 255 if value < 100 else 0).getbbox()
    assert bounds is not None and bounds[2] - bounds[0] > 450, (engine, bounds)
    producer_bounds[engine] = bounds
assert all(abs(a-b) <= 3 for a,b in zip(producer_bounds["mupdf"], producer_bounds["skia"])), producer_bounds
run("completed-chain-preview","mutool","draw","-r","144","-o",OUT/"completed-chain-%d.png",OUT/"classic-second.pdf")

(OUT / "manifest.json").write_text(json.dumps(dict(cases=records,
    versions=dict(pikepdf=pikepdf.__version__, pyhanko=pyhanko_version, openssl=run("openssl-version", "openssl", "version")),
    visual=visual, producer_text_bounds=producer_bounds,
    render_references="Independent page-level painting; render-only references do not preserve signature validity; producer Courier text also has a cross-engine bounds check",
    hashes={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.glob("*.pdf")},
    trust="synthetic local roots; no network; pyHanko diff for explicit-P fill/sign; P3 square uses exact independent projection; omitted-P default uses OpenSSL CMS plus field projection because pyHanko 0.37 requires P",
    private_key="synthetic-key.pem is disposable, local and excluded from repository"), indent=2) + "\n")
print(f"PASS {len(records)} field/MDP workflow cases and {len(visual)} exact pixel pairs, hierarchical/multi-Widget Unicode AP, diff validation and forbidden-change refusals")

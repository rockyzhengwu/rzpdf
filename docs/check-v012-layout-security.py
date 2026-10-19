"""Offline V12-04 gate; build v012_security_layout first. Existing tools only.
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-layout-security.py NEW_DIRECTORY
All credentials below are public synthetic test values, not user secrets.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import pikepdf
from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v012_security_layout"
USER, OWNER, NEW_USER, NEW_OWNER = "fixture-user", "fixture-owner", "replacement-user", "replacement-owner"
records, pixels, pictures = [], [], []


def run(label, *args, success=True, env=None):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240, env=env)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


def native(label, source, output, layout, policy, password="", flate=True, success=True, title=None):
    env = {k: v for k, v in os.environ.items() if not k.startswith("RZPDF_")}
    env.update(RZPDF_INPUT_PASSWORD=password, RZPDF_OUTPUT_USER_PASSWORD=NEW_USER,
               RZPDF_OUTPUT_OWNER_PASSWORD=NEW_OWNER)
    if title is not None:
        env["RZPDF_FIXTURE_TITLE"] = title
    return run(label, EXE, source, output, layout, policy, *(["flate"] if flate else []), env=env, success=success)


source = OUT / "pikepdf-classic.pdf"
with pikepdf.Pdf.new() as pdf:
    font = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1,
                                               BaseFont=pikepdf.Name.Helvetica))
    shared = pdf.make_indirect(pikepdf.Dictionary(Label=pikepdf.String("R6 private 中文")))
    shared.Self = shared
    pdf.Root.RzPrivate = shared
    pdf.Root.RzAlias = shared
    for i in range(2):
        page = pdf.add_blank_page(page_size=(320, 240))
        page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=font))
        page.Contents = pdf.make_stream((f"q 0.1 0.3 0.7 rg 20 20 80 40 re f Q\n"
            f"BT /F1 18 Tf 20 150 Td (Protected layout {i+1}) Tj ET\n").encode())
    pdf.docinfo.Title = "R6 layout projection"
    embedded = pdf.make_stream(b"attachment-secret\x00\xff")
    embedded.Type = pikepdf.Name.EmbeddedFile
    filespec = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Filespec, F=pikepdf.String("note.bin"), EF=pikepdf.Dictionary(F=embedded)))
    pdf.Root.Names = pikepdf.Dictionary(EmbeddedFiles=pdf.make_indirect(pikepdf.Dictionary(
        Names=pikepdf.Array([pikepdf.String("note.bin"), filespec]))))
    metadata = pdf.make_stream(b'<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/></x:xmpmeta>')
    metadata.Type, metadata.Subtype = pikepdf.Name.Metadata, pikepdf.Name.XML
    pdf.Root.Metadata = metadata
    pdf.save(source, force_version="1.7", object_stream_mode=pikepdf.ObjectStreamMode.disable,
             compress_streams=False, fix_metadata_version=False)


def external(label, mode, clear=False):
    output = OUT / (label + ".pdf")
    run(label + "-produce", "qpdf", f"--object-streams={mode}", "--encrypt", USER, OWNER, "256",
        *(["--cleartext-metadata"] if clear else []), "--", source, output)
    return output


classic = external("qpdf-classic-r6", "disable")
packed = external("qpdf-objstm-r6", "generate")
clear = external("qpdf-clear-metadata-r6", "generate", True)
direct = OUT / "synthetic-direct-encrypt-r6.pdf"
report = run("direct-input-xref", "qpdf", f"--password={OWNER}", "--show-xref", classic)
offsets = {int(n): (int(g), int(o)) for n, g, o in re.findall(r"(\d+)/(\d+): uncompressed; offset = (\d+)", report)}
with pikepdf.open(classic, password=OWNER, attempt_recovery=False) as pdf:
    trailer = pikepdf.Dictionary(pdf.trailer)
    encrypt_id = pdf.trailer.Encrypt.objgen[0]
    trailer.Encrypt = pikepdf.Dictionary(pdf.trailer.Encrypt)  # direct copy, not an indirect alias
    assert trailer.Encrypt.objgen == (0, 0)
    if "/Prev" in trailer:
        del trailer["/Prev"]
    data = bytearray(classic.read_bytes())
    start = len(data)
    size = int(trailer.Size)
    data.extend(f"\nxref\n0 {size}\n0000000000 65535 f \n".encode())
    # startxref points to 'xref', not the separator newline.
    start += 1
    for n in range(1, size):
        if n in offsets and n != encrypt_id:
            generation, offset = offsets[n]
            data.extend(f"{offset:010} {generation:05} n \n".encode())
        else:
            data.extend(b"0000000000 00001 f \n")
    data.extend(b"trailer\n" + trailer.unparse(resolved=True))
    data.extend(f"\nstartxref\n{start}\n%%EOF\n".encode())
    direct.write_bytes(data)


def structure(obj):
    return isinstance(obj, pikepdf.Stream) and obj.get("/Type") in (pikepdf.Name.XRef, pikepdf.Name.ObjStm)


def projection(pdf):
    encrypted_id = pdf.trailer.Encrypt.objgen if "/Encrypt" in pdf.trailer else (0, 0)
    values = {}
    for obj in pdf.objects:
        if obj.objgen in ((0, 0), encrypted_id) or structure(obj):
            continue
        if isinstance(obj, pikepdf.Stream):
            dictionary = pikepdf.Dictionary(obj.stream_dict)
            for key in ("/Length", "/Filter", "/DecodeParms"):
                if key in dictionary:
                    del dictionary[key]
            values[obj.objgen] = (dictionary.unparse(resolved=True), obj.read_bytes())
        else:
            values[obj.objgen] = obj.unparse(resolved=True)
    return values


def render(label, path, password):
    folder = OUT / label
    folder.mkdir()
    run(label, "mutool", "draw", "-r", "96", *(["-p", password] if password else []),
        "-o", folder / "page-%d.png", path)
    return folder


def wire_checks(pdf, data, layout, label):
    match = re.search(rb"startxref\s+(\d+)\s+%%EOF\s*$", data)
    offset = int(match[1])
    prefix = data[offset:]
    xref_id = int(prefix.split()[0])
    xref = pdf.get_object((xref_id, 0))
    assert xref.Type == pikepdf.Name.XRef
    rows = xref.read_bytes()
    size = int(xref.Size)
    assert list(xref.W) == [1, 8, 4] and list(xref.Index) == [0, size] and len(rows) == 13*size
    assert rows[13*xref_id] == 1 and int.from_bytes(rows[13*xref_id+1:13*xref_id+9], "big") == offset
    begin = offset + data[offset:].index(b"\nstream\n") + len(b"\nstream\n")
    assert data[begin:begin+len(rows)] == rows  # XRef wire bytes are never encrypted.
    if pdf.is_encrypted:
        enc_id, enc_gen = pdf.trailer.Encrypt.objgen
        assert enc_id > 0 and rows[13*enc_id] == 1
        assert int(pdf.trailer.Encrypt.R) == 6 and int(pdf.trailer.Encrypt.V) == 5
        assert data[int.from_bytes(rows[13*enc_id+1:13*enc_id+9], "big"):].startswith(f"{enc_id} {enc_gen} obj".encode())
        assert rows[13*pdf.Root.objgen[0]] == 1
    members, groups = set(), []
    for obj in pdf.objects:
        if not isinstance(obj, pikepdf.Stream) or obj.get("/Type") != pikepdf.Name.ObjStm:
            continue
        # Independent-object mode intentionally retains old containers; only
        # active generated membership is checked in packing mode.
        if layout != "objects":
            continue
        n, first = int(obj.N), int(obj.First)
        payload = obj.read_bytes()
        assert 0 < n <= 100 and 0 < first < len(payload) <= 1024*1024
        header = [int(x) for x in payload[:first].split()]
        assert len(header) == 2*n and header[1] == 0
        assert header[1::2] == sorted(set(header[1::2]))
        for index, number in enumerate(header[::2]):
            assert number not in members
            members.add(number)
            row = rows[13*number:13*(number+1)]
            assert row[0] == 2 and int.from_bytes(row[1:9], "big") == obj.objgen[0]
            assert int.from_bytes(row[9:], "big") == index
        container_offset = int.from_bytes(rows[13*obj.objgen[0]+1:13*obj.objgen[0]+9], "big")
        stream_start = container_offset + data[container_offset:].index(b"\nstream\n") + len(b"\nstream\n")
        wire = data[stream_start:stream_start+int(obj.Length)]
        if pdf.is_encrypted and pdf.trailer.Encrypt.get("/StmF") != pikepdf.Name.Identity:
            assert len(wire) >= 32 and (len(wire)-16) % 16 == 0
            assert wire != obj.read_raw_bytes()  # Decryption happens once, then Flate/member parsing.
        groups.append(dict(id=list(obj.objgen), count=n, decoded=len(payload), wire=len(wire)))
    if layout == "objects":
        assert members == {i for i in range(size) if rows[13*i] == 2} and groups
    if label.startswith("identity-"):
        assert str(pdf.docinfo.Title) == "identity-policy-title"
        assert rows[13*pdf.docinfo.objgen[0]] == 1  # mismatched defaults stay standalone
        marker = b"\xfe\xff" + "identity-policy-title".encode("utf-16-be")
        assert (marker in data) == (pdf.trailer.Encrypt.get("/StrF") == pikepdf.Name.Identity)
    return groups


def case(label, original, password, layout, policy, flate=True, pages=True):
    output = OUT / (label + ".pdf")
    native(label, original, output, layout, policy, password, flate)
    after_password = "" if policy == "remove" else (NEW_USER if policy.startswith("set") else password)
    run(label + "-qpdf-check", "qpdf", *([f"--password={after_password}"] if after_password else []), "--check", output)
    with pikepdf.open(original, password=password, attempt_recovery=False) as before, \
         pikepdf.open(output, password=after_password, attempt_recovery=False) as after:
        # QPDF's syntax checker can normalize an ADBE BaseVersion in memory.
        # Compare untouched read handles; run syntax checks on separate handles.
        expected = projection(before)
        if policy == "remove" and "/Encrypt" in before.trailer and before.trailer.Encrypt.objgen != (0, 0):
            expected[before.trailer.Encrypt.objgen] = before.trailer.Encrypt.unparse(resolved=True)
        assert projection(after) == expected, label
        assert after.is_encrypted == (policy != "remove")
        if after.is_encrypted:
            assert bytes(after.trailer.ID[0]) == bytes(before.trailer.ID[0])
            if policy == "preserve":
                assert after.trailer.Encrypt.unparse(resolved=True) == before.trailer.Encrypt.unparse(resolved=True)
            if policy.startswith("set"):
                assert bool(after.trailer.Encrypt.get("/EncryptMetadata", True)) == (policy != "set-clear")
        groups = wire_checks(after, output.read_bytes(), layout, label)
        assert len(after.pages) == len(before.pages)
        if after.is_encrypted and not bool(after.trailer.Encrypt.get("/EncryptMetadata", True)) and "/Metadata" in after.Root:
            # Clear metadata may still be Flate encoded: compare encoded raw
            # bytes, not decoded XML, to distinguish compression from encryption.
            assert after.Root.Metadata.read_raw_bytes() in output.read_bytes()
    for checked_path, checked_password in ((original, password), (output, after_password)):
        with pikepdf.open(checked_path, password=checked_password, attempt_recovery=False) as checked:
            assert not checked.check_pdf_syntax()
    if pages:
        reference = render(label + "-reference", original, password)
        actual = render(label + "-actual", output, after_password)
        for page in (1, 2):
            a = Image.open(reference / f"page-{page}.png").convert("RGB")
            b = Image.open(actual / f"page-{page}.png").convert("RGB")
            assert a.size == b.size and ImageChops.difference(a, b).getbbox() is None, (label, page)
            pixels.append(dict(case=label, page=page, exact_pixels=True))
            pictures.append((label, page, b.copy()))
    records.append(dict(case=label, source=original.name, output=output.name, policy=policy, layout=layout, flate=flate, groups=groups))
    return output


native_packed = None
for layout in ("stream", "objects"):
    for policy in ("set", "set-clear"):
        for flate in (False, True):
            result = case(f"plain-{layout}-{policy}-{'flate' if flate else 'raw'}", source, "", layout, policy, flate)
            if layout == "objects" and policy == "set" and flate:
                native_packed = result

for original in (classic, packed, clear, direct):
    for layout in ("stream", "objects"):
        for role, password, policy in (("user",USER,"preserve"), ("owner",OWNER,"preserve"),
                                        ("owner",OWNER,"remove"), ("owner",OWNER,"set")):
            case(f"{original.stem}-{layout}-{role}-{policy}", original, password, layout, policy)
        for policy in ("remove", "set"):
            refused = OUT / f"{original.stem}-{layout}-user-{policy}-refused.pdf"
            native(refused.stem, original, refused, layout, policy, USER, success=False)
            assert not refused.exists()
        refused = OUT / f"{original.stem}-{layout}-wrong-refused.pdf"
        native(refused.stem, original, refused, layout, "preserve", "wrong", success=False)
        assert not refused.exists()

for layout in ("stream", "objects"):
    for policy in ("preserve", "remove", "set"):
        case(f"native-r6-{layout}-{policy}", native_packed, NEW_OWNER, layout, policy)

# Identity defaults: use a seed with no body strings/streams, so selector-only
# edits do not mislabel pre-existing ciphertext. External qpdf checks each seed.
empty = OUT / "empty.pdf"
with pikepdf.Pdf.new() as pdf:
    pdf.save(empty, force_version="2.0", object_stream_mode=pikepdf.ObjectStreamMode.disable)
seed = OUT / "identity-seed.pdf"
native("identity-seed", empty, seed, "classic", "set", flate=False)
for field in ("StrF", "StmF"):
    data = seed.read_bytes()
    match = re.search(rb"startxref\s+(\d+)\s+%%EOF\s*$", data)
    old_xref = int(match[1])
    needle, replacement = f"/{field} /StdCF".encode(), f"/{field} /Identity".encode()
    assert data.count(needle) == 1
    data = data.replace(needle, replacement)
    footer = data.rfind(b"startxref")
    data = data[:footer] + f"startxref\n{old_xref+len(replacement)-len(needle)}\n%%EOF\n".encode()
    original = OUT / f"identity-{field}.pdf"
    original.write_bytes(data)
    run(f"identity-{field}-source-check", "qpdf", f"--password={NEW_OWNER}", "--check", original)
    populated = OUT / f"identity-{field}-populated.pdf"
    native(f"identity-{field}-populate", original, populated, "classic", "preserve", NEW_OWNER,
           flate=False, title="identity-policy-title")
    case(f"identity-{field}-preserve", populated, NEW_OWNER, "objects", "preserve", pages=False)

legacy = OUT / "legacy-r5.pdf"
run("legacy-produce", "qpdf", "--encrypt", USER, OWNER, "256", "--force-R5", "--", source, legacy)
for layout in ("stream", "objects"):
    refused = OUT / f"legacy-{layout}-refused.pdf"
    native(refused.stem, legacy, refused, layout, "preserve", OWNER, success=False)
    assert not refused.exists()
existing = OUT / "existing.pdf"
existing.write_bytes(b"keep existing target")
native("no-overwrite", source, existing, "objects", "set", success=False)
assert existing.read_bytes() == b"keep existing target"

for sheet, start in enumerate(range(0, len(pictures), 16), 1):
    canvas = Image.new("RGB", (1760, 1400), "#eeeeee")
    draw = ImageDraw.Draw(canvas)
    for index, (label, page, picture) in enumerate(pictures[start:start+16]):
        x, y = (index % 4)*440, (index // 4)*350
        picture.thumbnail((426, 320))
        canvas.paste(picture, (x+7, y+24))
        draw.text((x+7,y+5), f"{label} / {page}", fill="black")
    canvas.save(OUT / f"visual-review-{sheet}.png")

(OUT / "manifest.json").write_text(json.dumps(dict(cases=records,pixels=pixels,
    versions=dict(pikepdf=pikepdf.__version__,qpdf=run("qpdf-version","qpdf","--version"),
                  mupdf=run("mupdf-version","mutool","-v")),
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.glob("*.pdf")},
    provenance="Synthetic pikepdf; independent qpdf R6 classic/ObjStm/clear metadata; explicit direct Encrypt and Identity defaults; native R6 roundtrip.",
    trust="Public test credentials only; no network, installs, cracking, signatures or fuzz. pikepdf uses qpdf, not a second crypto engine."),indent=2)+"\n")
print(f"PASS {len(records)} R6 layout cases, {len(pixels)} exact MuPDF page pairs, permissions/legacy/host refusals")

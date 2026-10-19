"""Offline V12-06 relation preservation gate; existing tools only.
Build v012_preservation and v012_layout first, then:
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-preservation.py NEW_DIRECTORY
All documents and passwords are synthetic. No network, installs, Poppler or fuzz.
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
BIN = ROOT / "target/debug/examples"
USER, OWNER = "fixture-user", "fixture-owner"
PACKET = b'<x:xmpmeta xmlns:x="adobe:ns:meta/">\n  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/>\n</x:xmpmeta>\n'
PAYLOAD = b"associated file payload\x00\xff"
cases, pixels, blanks, pictures = [], [], [], []


def run(label, *args, password="", success=True):
    env = {k: v for k, v in os.environ.items() if not k.startswith("RZPDF_")}
    env["RZPDF_INPUT_PASSWORD"] = password
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240, env=env)
    (OUT/(label+".log")).write_text(result.stdout+result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout+result.stderr


def last(data):
    match = re.search(rb"startxref\s+(\d+)\s+%%EOF\s*$", data)
    assert match
    return int(match[1])


def add_trailer_roots(path, password="", owner=False):
    """Add extension roots without rewriting ciphertext or object/stream offsets."""
    data = path.read_bytes()
    position = last(data)
    with pikepdf.open(path, password=password, attempt_recovery=False) as pdf:
        if data[position:].startswith(b"xref"):
            dictionary = pikepdf.Dictionary(pdf.trailer)
        else:
            self_id = tuple(int(n) for n in data[position:].split()[:2])
            dictionary = pikepdf.Dictionary(pdf.get_object(self_id).stream_dict)
        dictionary.RzTrailer = pdf.Root.RzPrivate
        dictionary.RzScalar = pikepdf.Array([1.25, True, None, pikepdf.Name.Extension])
        if owner:
            dictionary.RzOwner = pdf.pages[0].obj
        if data[position:].startswith(b"xref"):
            marker = data.rfind(b"\ntrailer")
            assert marker > position
            data = data[:marker]+b"\ntrailer\n"+dictionary.unparse(resolved=True)+f"\nstartxref\n{position}\n%%EOF\n".encode()
        else:
            # Only the final xref stream dictionary grows; all recorded offsets,
            # including its self-entry, remain unchanged. Its payload is untouched.
            payload = data.index(b"\nstream\n", position)+8
            data = (data[:position]+f"{self_id[0]} {self_id[1]} obj\n".encode()+dictionary.unparse(resolved=True)
                    +b"\nstream\n"+data[payload:])
    path.write_bytes(data)


def fixture(path, *, info_owner=False, private_owner=False, layers=False, trailer_roots=True):
    with pikepdf.Pdf.new() as pdf:
        shared = pdf.make_indirect(pikepdf.Dictionary(Label=pikepdf.String("private 数据")))
        shared.Self = shared
        shared.Opaque = pikepdf.Array([None, True, 1.25, pikepdf.Name.Extension])
        pdf.Root.RzPrivate = pdf.Root.RzAlias = shared
        font = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1,
            BaseFont=pikepdf.Name.Helvetica))
        embedded = pdf.make_stream(PAYLOAD)
        embedded.Type, embedded.Subtype = pikepdf.Name.EmbeddedFile, pikepdf.Name("/application/octet-stream")
        spec = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Filespec, F=pikepdf.String("note.bin"),
            UF=pikepdf.String("note.bin"), AFRelationship=pikepdf.Name.Data,
            Desc=pikepdf.String("association metadata"), EF=pikepdf.Dictionary(F=embedded), Private=shared))
        packet = pdf.make_stream(PACKET)
        packet.Type, packet.Subtype = pikepdf.Name.Metadata, pikepdf.Name.XML
        pdf.Root.Metadata = packet
        pdf.Root.AF = pikepdf.Array([spec])
        pdf.Root.Names = pikepdf.Dictionary(EmbeddedFiles=pikepdf.Dictionary(Names=pikepdf.Array([pikepdf.String("note.bin"), spec])))
        for i in range(2):
            page = pdf.add_blank_page(page_size=(320, 240))
            page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=font))
            page.Contents = pdf.make_stream((f"0.1 0.3 0.7 rg 20 20 60 40 re f\n"
                f"BT /F1 18 Tf 20 150 Td (Preserved page {i+1}) Tj ET\n").encode())
            page.RzPrivate = page.RzAlias = shared
            page.AF = pikepdf.Array([spec])
            page.Metadata = packet
            page.PieceInfo = pikepdf.Dictionary(Rz=pikepdf.Dictionary(LastModified=pikepdf.String("D:20261019000000Z"), Private=shared))
        pdf.docinfo.Title = "preservation input"
        pdf.docinfo.Private = shared
        if info_owner:
            pdf.docinfo.Owner = pdf.pages[0].obj
        if private_owner:
            shared.Owner = pdf.pages[1].obj
        if layers:
            group = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.OCG, Name=pikepdf.String("visible layer"), Private=shared))
            pdf.Root.OCProperties = pikepdf.Dictionary(OCGs=pikepdf.Array([group]), D=pikepdf.Dictionary(
                BaseState=pikepdf.Name.ON, Order=pikepdf.Array([group]), RzPrivate=pikepdf.String("config metadata")))
            tree = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.StructTreeRoot, K=pikepdf.Array()))
            element = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.StructElem, S=pikepdf.Name.Document,
                P=tree, K=pikepdf.Array(), ActualText=pikepdf.String("declared text")))
            tree.K = pikepdf.Array([element])
            pdf.Root.StructTreeRoot = tree
            pdf.Root.MarkInfo = pikepdf.Dictionary(Marked=True)
        pdf.save(path, force_version="2.0", object_stream_mode=pikepdf.ObjectStreamMode.disable,
            compress_streams=False, fix_metadata_version=False)
    if trailer_roots:
        add_trailer_roots(path)


def structural(obj):
    return isinstance(obj, pikepdf.Stream) and obj.get("/Type") in (pikepdf.Name.XRef, pikepdf.Name.ObjStm)


def projection(pdf, excluded):
    values = {}
    for obj in pdf.objects:
        if obj.objgen in excluded or obj.objgen == (0, 0) or structural(obj):
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


def business_trailer(pdf):
    return {key: value.unparse() if hasattr(value, "unparse") else value for key, value in pdf.trailer.items()
            if key not in ("/Size", "/Prev", "/XRefStm", "/Type", "/W", "/Index", "/Filter", "/DecodeParms", "/Length", "/Encrypt", "/ID", "/Info")}


def associations(pdf, imported=False):
    spec_ids, shared_ids = set(), set()
    for page in pdf.pages:
        if "/RzPrivate" not in page.obj:  # native destination's preexisting blank page
            continue
        shared = page.RzPrivate
        assert shared.objgen == page.RzAlias.objgen == shared.Self.objgen
        assert str(shared.Label) == "private 数据"
        assert list(shared.Opaque) == [None, True, 1.25, pikepdf.Name.Extension]
        assert page.PieceInfo.Rz.Private.objgen == shared.objgen
        assert str(page.PieceInfo.Rz.LastModified) == "D:20261019000000Z"
        assert page.Metadata.read_bytes() == PACKET
        spec = page.AF[0]
        assert spec.AFRelationship == pikepdf.Name.Data and str(spec.Desc) == "association metadata"
        assert str(spec.F) == str(spec.UF) == "note.bin"
        assert spec.EF.F.read_bytes() == PAYLOAD and spec.Private.objgen == shared.objgen
        shared_ids.add(shared.objgen)
        spec_ids.add(spec.objgen)
    assert len(shared_ids) == len(spec_ids) == 1
    if imported:
        assert "/RzPrivate" not in pdf.Root and "/AF" not in pdf.Root
        assert "/Names" not in pdf.Root or "/EmbeddedFiles" not in pdf.Root.Names
        assert pdf.Root.Metadata.read_bytes() == b"target packet"
        assert str(pdf.docinfo.Title) == "target title"
    else:
        assert pdf.Root.RzPrivate.objgen in shared_ids
        assert pdf.Root.RzAlias.objgen in shared_ids
        if "/RzTrailer" in pdf.trailer:
            assert pdf.trailer.RzTrailer.objgen in shared_ids
        assert pdf.Root.AF[0].objgen in spec_ids
        assert pdf.Root.Metadata.read_bytes() == PACKET
        assert pdf.docinfo.Private.objgen in shared_ids


def render(label, path, password):
    folder = OUT / label
    folder.mkdir()
    log = run(label, "mutool", "draw", "-r", "96", *(["-p", password] if password else []), "-o", folder/"page-%d.png", path)
    assert not re.search(r"repair|reconstruct|format error|cannot load", log, re.I)
    return folder


references = {}
def case(source, password, action, layout, order, *, flate=False):
    label = f"{source.stem}-{action}-{layout}"+("-flate" if flate else "")
    output = OUT/(label+".pdf")
    before = source.read_bytes()
    role = OWNER if password and action == "edit" and layout.endswith("stream") else password
    if flate:
        assert not password and action == "preserve"
        run(label+"-native", BIN/"v012_layout", source, output, layout, "flate")
    else:
        run(label+"-native", BIN/"v012_preservation", source, output, action, layout, password=role)
    assert source.read_bytes() == before
    data = output.read_bytes()
    if layout.startswith("incremental"):
        assert data.startswith(before)
    run(label+"-qpdf", "qpdf", *([f"--password={password}"] if password else []), "--check", output)
    with pikepdf.open(source, password=password, attempt_recovery=False) as old, pikepdf.open(output, password=password, attempt_recovery=False) as pdf:
        assert len(pdf.pages) == len(order)
        imported = action in ("import", "select", "import-retry")
        associations(pdf, imported)
        encrypt_ids = {old.trailer.Encrypt.objgen, pdf.trailer.Encrypt.objgen} if password else set()
        if action in ("preserve", "edit"):
            excluded = set(encrypt_ids)
            if action == "edit":
                excluded.update((old.trailer.Info.objgen, pdf.trailer.Info.objgen))
                current_info = pikepdf.Dictionary(pdf.trailer.Info)
                original_info = pikepdf.Dictionary(old.trailer.Info)
                del current_info["/Title"], original_info["/Title"]
                assert current_info.unparse(resolved=True) == original_info.unparse(resolved=True)
                assert str(pdf.docinfo.Title) == "preservation edit"
            assert projection(pdf, excluded) == projection(old, excluded)
            assert business_trailer(pdf) == business_trailer(old)
            if "/StructTreeRoot" in old.Root:
                assert pdf.Root.StructTreeRoot.unparse(resolved=True) == old.Root.StructTreeRoot.unparse(resolved=True)
                assert pdf.Root.OCProperties.unparse(resolved=True) == old.Root.OCProperties.unparse(resolved=True)
        elif action in ("copy", "delete", "delete-retry"):
            # Existing non-page business objects must retain IDs and values.
            excluded = {p.obj.objgen for p in old.pages} | {old.Root.Pages.objgen}
            expected = projection(old, excluded)
            actual = projection(pdf, excluded)
            assert all(actual[key] == value for key, value in expected.items())
            assert business_trailer(pdf) == business_trailer(old)
            if action == "delete-retry" and "/RzOwner" in pdf.trailer:
                assert pdf.trailer.RzOwner.objgen == pdf.pages[0].obj.objgen
        if action == "import-retry":
            assert pdf.pages[0].RzPrivate.Owner.objgen == pdf.pages[1].obj.objgen
        if password:
            assert pdf.trailer.Encrypt.unparse(resolved=True) == old.trailer.Encrypt.unparse(resolved=True)
            assert bytes(pdf.trailer.ID[0]) == bytes(old.trailer.ID[0])
            with pikepdf.open(output, password=OWNER, attempt_recovery=False) as owner:
                assert len(owner.pages) == len(order) and not owner.get_warnings()
        assert not pdf.get_warnings()
    folder = render(label+"-actual", output, password)
    if source not in references:
        run(source.stem+"-input-check", "qpdf", *([f"--password={password}"] if password else []), "--check", source)
        references[source] = render(source.stem+"-reference", source, password)
    for number, original in enumerate(order, 1):
        filename = folder/f"page-{number}.png"
        with Image.open(filename) as image:
            image = image.convert("RGB")
            if original is None:
                assert ImageChops.difference(image, Image.new("RGB", image.size, "white")).getbbox() is None
                blanks.append((label, number))
            else:
                with Image.open(references[source]/f"page-{original+1}.png") as expected:
                    assert image.size == expected.size
                    assert ImageChops.difference(image, expected.convert("RGB")).getbbox() is None
                pixels.append((label, number, original+1))
        pictures.append((label+f" / {number}", filename))
    cases.append(dict(case=label, action=action, source=source.name, password_role="synthetic R6" if password else "none",
        source_sha256=hashlib.sha256(before).hexdigest(), output_sha256=hashlib.sha256(data).hexdigest(), pages=len(order)))


classic = OUT/"classic.pdf"
fixture(classic)
clean = OUT/"clean-container-seed.pdf"
# Distinct input profile: business private keys/AF/XMP are present, but no
# unknown fields on the structural xref container. Repacking the latter remains
# conservatively unsupported and is tested as an explicit refusal below.
fixture(clean, trailer_roots=False)
packed = OUT/"qpdf-objstm.pdf"
run("packed-produce", "qpdf", "--object-streams=generate", clean, packed)
for source in (classic, packed):
    for layout in ("classic", "stream", "objects"):
        for flate in (False, True):
            case(source, "", "preserve", layout, [0, 1], flate=flate)
        case(source, "", "copy", layout, [0, 1, 0])
        case(source, "", "delete", layout, [1])
        case(source, "", "import", layout, [1, 0, None])
    case(source, "", "select", "objects", [0])
    for layout in ("incremental-classic", "incremental-stream"):
        case(source, "", "edit", layout, [0, 1])

for label, mode, clear in (("r6-classic", "disable", False), ("r6-objstm", "generate", False), ("r6-clear", "generate", True)):
    protected = OUT/(label+".pdf")
    seed = classic if mode == "disable" else clean
    run(label+"-produce", "qpdf", f"--object-streams={mode}", "--encrypt", USER, OWNER, "256",
        *(["--cleartext-metadata"] if clear else []), "--", seed, protected)
    if mode == "disable":
        add_trailer_roots(protected, USER)
    for layout in ("classic", "stream", "objects"):
        case(protected, USER, "preserve", layout, [0, 1])
    for layout in ("incremental-classic", "incremental-stream"):
        case(protected, USER, "edit", layout, [0, 1])

layers = OUT/"structure-ocg.pdf"
fixture(layers, layers=True)
for layout in ("classic", "stream", "objects"):
    case(layers, "", "preserve", layout, [0, 1])
for layout in ("incremental-classic", "incremental-stream"):
    case(layers, "", "edit", layout, [0, 1])
refused = OUT/"layers-refused.pdf"
run("layers-import-refused", BIN/"v012_preservation", layers, refused, "import", "objects", success=False)
assert not refused.exists()

for label, info in (("info-owner", True), ("trailer-owner", False)):
    source = OUT/(label+".pdf")
    fixture(source, info_owner=info)
    if not info:
        add_trailer_roots(source, owner=True)
    refused = OUT/(label+"-refused.pdf")
    run(label+"-delete-refused", BIN/"v012_preservation", source, refused, "delete", "objects", success=False)
    assert not refused.exists()
    case(source, "", "delete-retry", "objects", [0])

owner = OUT/"private-owner.pdf"
fixture(owner, private_owner=True)
refused = OUT/"selection-refused.pdf"
run("selection-refused", BIN/"v012_preservation", owner, refused, "select", "objects", success=False)
assert not refused.exists()
case(owner, "", "import-retry", "objects", [0, 1, None])

opaque = OUT/"xref-private-fields.pdf"
opaque.write_bytes(packed.read_bytes())
add_trailer_roots(opaque)
for action in ("preserve", "copy", "delete"):
    refused = OUT/("xref-private-"+action+"-refused.pdf")
    run(refused.stem, BIN/"v012_preservation", opaque, refused, action, "objects", success=False)
    assert not refused.exists()
for layout in ("classic", "stream"):
    case(opaque, "", "preserve", layout, [0, 1])
case(opaque, "", "edit", "incremental-stream", [0, 1])

# Real structural containers, not arbitrary strings named ObjStm/XRef. A new
# conventional revision adds an ordinary page reference to the old container.
for kind in (pikepdf.Name.ObjStm, pikepdf.Name.XRef):
    source = OUT/(str(kind)[1:]+"-owner.pdf")
    data = packed.read_bytes()
    with pikepdf.open(packed, attempt_recovery=False) as pdf:
        container = next(obj for obj in pdf.objects if structural(obj) and obj.Type == kind)
        page = pikepdf.Dictionary(pdf.pages[1].obj)
        page.PrivateContainer = container
        number, generation = pdf.pages[1].obj.objgen
        body = bytearray(data+b"\n")
        position = len(body)
        body.extend(f"{number} {generation} obj\n".encode()+page.unparse(resolved=True)+b"\nendobj\n")
        offset = len(body)
        trailer = pikepdf.Dictionary(pdf.trailer)
        for key in ("/Type", "/W", "/Index", "/Length", "/Filter", "/DecodeParms", "/XRefStm"):
            if key in trailer:
                del trailer[key]
        trailer.Prev = last(data)
        body.extend(f"xref\n0 1\n0000000000 65535 f \n{number} 1\n{position:010} {generation:05} n \ntrailer\n".encode()+trailer.unparse(resolved=True))
        body.extend(f"\nstartxref\n{offset}\n%%EOF\n".encode())
        source.write_bytes(body)
    run(source.stem+"-check", "qpdf", "--check", source)
    for action in ("import", "delete"):
        refused = OUT/(source.stem+"-"+action+"-refused.pdf")
        run(refused.stem, BIN/"v012_preservation", source, refused, action, "classic", success=False)
        assert not refused.exists()

for sheet_no, start in enumerate(range(0, len(pictures), 16), 1):
    sheet = Image.new("RGB", (1760, 1400), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    for i, (label, filename) in enumerate(pictures[start:start+16]):
        x, y = (i%4)*440, (i//4)*350
        with Image.open(filename) as image:
            image.thumbnail((426, 320))
            sheet.paste(image, (x+7, y+24))
        draw.text((x+7, y+5), label, fill="black")
    sheet.save(OUT/f"visual-review-{sheet_no}.png")

(OUT/"manifest.json").write_text(json.dumps(dict(cases=cases, pixel_pairs=pixels, blank_pages=blanks,
    rendered_pages=len(pictures), versions=dict(pikepdf=pikepdf.__version__, qpdf=run("qpdf-version", "qpdf", "--version"),
        mupdf=run("mupdf-version", "mutool", "-v")),
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.glob("*.pdf")},
    provenance="Original synthetic pikepdf fixtures, qpdf ObjStm/R6, explicit trailer-root/structural-container references. No third-party document content.",
    scope="Representation preservation, not validation of private schemas, complete AF semantics, Tagged PDF or PDF/UA conformance."),indent=2)+"\n")
print(f"PASS cases={len(cases)} pixel_pairs={len(pixels)} blank_pages={len(blanks)} rendered_pages={len(pictures)}")

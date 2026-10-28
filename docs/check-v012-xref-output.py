"""Offline V12-01/02 gate; build v012_layout first. Existing environment only:
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-xref-output.py NEW_DIRECTORY
No downloads, installation, credentials, Poppler or fuzz.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pikepdf
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=False)
EXE = ROOT / "target/debug/examples/v012_layout"


def run(label, *args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


source = OUT / "pikepdf-classic.pdf"
with pikepdf.Pdf.new() as pdf:
    font = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1,
                                               BaseFont=pikepdf.Name.Helvetica))
    shared = pdf.make_indirect(pikepdf.Dictionary(Label=pikepdf.String("preserve 中文")))
    shared.Self = shared
    pdf.Root.RzPrivate = shared
    pdf.Root.RzAlias = shared
    for i in range(2):
        page = pdf.add_blank_page(page_size=(320, 240))
        page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=font))
        page.Contents = pdf.make_stream((f"q 0.1 0.3 0.7 rg 20 20 80 40 re f Q\n"
            f"BT /F1 18 Tf 20 150 Td (Layout page {i+1}) Tj ET\n").encode())
    pdf.docinfo.Title = "xref output fixture"
    embedded = pdf.make_stream(b"synthetic attachment\x00\xff")
    embedded.Type = pikepdf.Name.EmbeddedFile
    filespec = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Filespec, F=pikepdf.String("note.txt"),
                                EF=pikepdf.Dictionary(F=embedded)))
    pdf.Root.Names = pikepdf.Dictionary(EmbeddedFiles=pdf.make_indirect(
        pikepdf.Dictionary(Names=pikepdf.Array([pikepdf.String("note.txt"), filespec]))))
    metadata = pdf.make_stream(b'<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/></x:xmpmeta>')
    metadata.Type = pikepdf.Name.Metadata
    metadata.Subtype = pikepdf.Name.XML
    pdf.Root.Metadata = metadata
    pdf.save(source, force_version="1.7", object_stream_mode=pikepdf.ObjectStreamMode.disable,
             compress_streams=False, fix_metadata_version=False)

objstm = OUT / "qpdf-objstm.pdf"
run("qpdf-produce", "qpdf", "--object-streams=generate", source, objstm)
mupdf = OUT / "mupdf-classic.pdf"
run("mupdf-produce", "mutool", "clean", source, mupdf)
stream_source = OUT / "rzpdf-stream.pdf"
run("rzpdf-produce", EXE, source, stream_source, "stream")

# Add a traditional section referring to the complete stream, rather than
# rewriting stream members. Both the table and supplemental stream are checked
# independently by qpdf. This synthetic input has no compressed members.
hybrid = OUT / "synthetic-hybrid.pdf"
data = stream_source.read_bytes()
cut = data.rfind(b"\nstartxref\n")
assert cut >= 0
with pikepdf.open(stream_source) as pdf:
    trailer = pikepdf.Dictionary(pdf.trailer)
    size = int(trailer.Size)
    records = pdf.get_object((size-1, 0)).read_bytes()
    assert len(records) == size*13
    offset = int.from_bytes(records[(size-1)*13+1:(size-1)*13+9], "big")
    for key in ("/Type", "/W", "/Index", "/Length", "/Filter", "/DecodeParms", "/Prev"):
        if key in trailer:
            del trailer[key]
    trailer.XRefStm = offset
    body = bytearray(data[:cut+1])
    table_offset = len(body)
    body.extend(f"xref\n0 {size}\n".encode())
    for i in range(size):
        row = records[i*13:(i+1)*13]
        assert row[0] in (0, 1)
        field, generation = int.from_bytes(row[1:9], "big"), int.from_bytes(row[9:], "big")
        state = "n" if row[0] else "f"
        body.extend(f"{field:010} {generation:05} {state} \n".encode())
    body.extend(b"trailer\n" + trailer.unparse(resolved=True))
    body.extend(f"\nstartxref\n{table_offset}\n%%EOF\n".encode())
    hybrid.write_bytes(body)


def projection(pdf):
    result = {}
    for obj in pdf.objects:
        if obj.objgen == (0, 0):
            continue
        if isinstance(obj, pikepdf.Stream):
            dictionary = pikepdf.Dictionary(obj.stream_dict)
            for key in ("/Length", "/Filter", "/DecodeParms"):
                if key in dictionary:
                    del dictionary[key]
            result[obj.objgen] = (dictionary.unparse(resolved=True), obj.read_bytes())
        else:
            result[obj.objgen] = obj.unparse(resolved=True)
    return result


def render(label, path):
    folder = OUT / label
    folder.mkdir()
    run(label, "mutool", "draw", "-r", "96", "-o", folder/"page-%d.png", path)
    return folder


records = []
pixels = []
for original in (source, objstm, mupdf, stream_source, hybrid):
    run(original.stem+"-source-check", "qpdf", "--check", original)
    reference = render(original.stem+"-reference", original)
    for compress in (False, True):
        label = original.stem + ("-flate" if compress else "-preserve")
        output = OUT / (label + ".pdf")
        run(label, EXE, original, output, "stream", *(["flate"] if compress else []))
        run(label+"-check", "qpdf", "--check", output)
        run(label+"-xref", "qpdf", "--show-xref", output)
        with pikepdf.open(original) as before, pikepdf.open(output) as after:
            expected, actual = projection(before), projection(after)
            assert all(actual[key] == value for key,value in expected.items()), label
            assert len(actual) == len(expected)+1, label
            trailer = after.trailer
            assert trailer.Type == pikepdf.Name.XRef
            assert list(trailer.W) == [1,8,4]
            assert list(trailer.Index) == [0,int(trailer.Size)]
            assert not any(key in trailer for key in ("/Prev","/XRefStm","/Filter","/Encrypt"))
            fresh = set(actual)-set(expected)
            assert fresh == {(int(trailer.Size)-1,0)}
            assert len(after.pages) == len(before.pages) == 2
        actual_render = render(label+"-actual", output)
        for page in (1,2):
            a = Image.open(reference/f"page-{page}.png").convert("RGB")
            b = Image.open(actual_render/f"page-{page}.png").convert("RGB")
            assert a.size == b.size and ImageChops.difference(a,b).getbbox() is None, (label,page)
            pixels.append(dict(case=label,page=page,exact_pixels=True))
        records.append(dict(case=label, source=original.name, output=output.name))

linearized = OUT / "linearized.pdf"
run("linearized-produce", "qpdf", "--linearize", source, linearized)
run("linearized-check", "qpdf", "--check", linearized)
ordinary = OUT / "linearized-to-ordinary.pdf"
run("linearized-rewrite", EXE, linearized, ordinary, "stream")
assert "File is not linearized" in run("ordinary-check", "qpdf", "--check", ordinary)
with pikepdf.Pdf.open(ordinary) as after, pikepdf.Pdf.open(linearized) as before:
    assert not after.is_linearized
    assert len(after.pages) == len(before.pages)
    for a, b in zip(after.pages, before.pages):
        assert a.Contents.read_bytes() == b.Contents.read_bytes()

existing = OUT / "existing.pdf"
existing.write_bytes(b"keep existing target")
run("no-overwrite", EXE, source, existing, "stream", success=False)
assert existing.read_bytes() == b"keep existing target"
(OUT / "manifest.json").write_text(json.dumps(dict(cases=records,pixels=pixels,
    versions=dict(pikepdf=pikepdf.__version__,qpdf=run("qpdf-version","qpdf","--version")),
    provenance="pikepdf authored; qpdf ObjStm and linearized-to-ordinary rewrite; MuPDF rewrite; rzpdf xref stream; explicit synthetic hybrid",
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.glob("*.pdf")},
    trust="No signatures, credentials, network, installs or fuzz. Projection compares decoded stream content and non-encoding dictionary keys."),indent=2)+"\n")
print(f"PASS {len(records)} xref-stream rewrites, {len(pixels)} exact MuPDF page pairs, independent projections and no-overwrite")

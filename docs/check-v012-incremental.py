"""Offline V12-05 gate. Build v012_layout, v012_security_layout and v012_incremental first.
PYTHONPATH=tmp/pdfs/v05-verifiers:tmp/pdfs/python python3 docs/check-v012-incremental.py NEW_DIRECTORY
Existing qpdf, pikepdf, MuPDF and Pillow only. Public synthetic credentials only.
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
records, pixels, pictures = [], [], []


def run(label, *args, password="", success=True):
    env = {k: v for k, v in os.environ.items() if not k.startswith("RZPDF_")}
    env.update(RZPDF_INPUT_PASSWORD=password, RZPDF_OUTPUT_USER_PASSWORD=USER,
               RZPDF_OUTPUT_OWNER_PASSWORD=OWNER)
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=240, env=env)
    (OUT / (label + ".log")).write_text(result.stdout + result.stderr)
    assert (result.returncode == 0) == success, (label, result.returncode, result.stderr)
    return result.stdout + result.stderr


def last(data):
    match = re.search(rb"startxref\s+(\d+)\s+%%EOF\s*$", data)
    assert match
    return int(match[1])


def structural(obj):
    return isinstance(obj, pikepdf.Stream) and obj.get("/Type") in (pikepdf.Name.XRef, pikepdf.Name.ObjStm)


def projection(pdf, excluded):
    result = {}
    encrypt = pdf.trailer.Encrypt.objgen if "/Encrypt" in pdf.trailer else (0, 0)
    for obj in pdf.objects:
        if obj.objgen in excluded or obj.objgen in ((0, 0), encrypt) or structural(obj):
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


def render(label, path, password):
    directory = OUT / label
    directory.mkdir()
    run(label, "mutool", "draw", "-r", "96", *(["-p", password] if password else []),
        "-o", directory / "page-%d.png", path)
    return directory


source = OUT / "pikepdf-classic.pdf"
with pikepdf.Pdf.new() as pdf:
    font = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Font, Subtype=pikepdf.Name.Type1,
                                               BaseFont=pikepdf.Name.Helvetica))
    private = pdf.make_indirect(pikepdf.Dictionary(Label=pikepdf.String("private shared 中文")))
    private.Self = private
    pdf.Root.RzPrivate, pdf.Root.RzAlias = private, private
    pdf.Root.RzMutation = pdf.make_indirect(pikepdf.String("private-before"))
    for i in range(2):
        page = pdf.add_blank_page(page_size=(320, 240))
        page.Resources = pikepdf.Dictionary(Font=pikepdf.Dictionary(F1=font))
        page.Contents = pdf.make_stream((f"q 0.1 0.3 0.7 rg 20 20 80 40 re f Q\n"
            f"BT /F1 18 Tf 20 150 Td (Incremental page {i+1}) Tj ET\n").encode())
    pdf.docinfo.Title = "private-before"
    embedded = pdf.make_stream(b"synthetic attachment\x00\xff")
    embedded.Type = pikepdf.Name.EmbeddedFile
    filespec = pdf.make_indirect(pikepdf.Dictionary(Type=pikepdf.Name.Filespec, F=pikepdf.String("note.bin"),
        EF=pikepdf.Dictionary(F=embedded)))
    pdf.Root.Names = pikepdf.Dictionary(EmbeddedFiles=pdf.make_indirect(pikepdf.Dictionary(
        Names=pikepdf.Array([pikepdf.String("note.bin"), filespec]))))
    metadata = pdf.make_stream(b'<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/></x:xmpmeta>')
    metadata.Type, metadata.Subtype = pikepdf.Name.Metadata, pikepdf.Name.XML
    pdf.Root.Metadata = metadata
    pdf.save(source, force_version="1.7", object_stream_mode=pikepdf.ObjectStreamMode.disable,
             compress_streams=False, fix_metadata_version=False)

packed, mupdf, stream = [OUT / (name + ".pdf") for name in ("qpdf-objstm", "mupdf-classic", "rzpdf-stream")]
run("qpdf-produce", "qpdf", "--object-streams=generate", source, packed)
run("mupdf-produce", "mutool", "clean", source, mupdf)
run("stream-produce", BIN / "v012_layout", source, stream, "stream")

# One revision with a conventional table and a same-revision stream supplement.
# Its physical last section is the table, NOT its XRefStm target.
def hybrid_from(source, output, password=""):
    data = source.read_bytes()
    with pikepdf.open(source, password=password, attempt_recovery=False) as pdf:
        trailer = pikepdf.Dictionary(pdf.trailer)
        size = int(trailer.Size)
        self_id = int(data[last(data):].split()[0])
        rows = pdf.get_object((self_id, 0)).read_bytes()
        assert len(rows) == size*13
        for key in ("/Type", "/W", "/Index", "/Length", "/Filter", "/DecodeParms", "/Prev"):
            if key in trailer:
                del trailer[key]
        trailer.XRefStm = last(data)
        body = bytearray(data[:data.rfind(b"\nstartxref\n")+1])
        position = len(body)
        body.extend(b"xref\n")
        # Omit compressed members from the conventional section. Free-zero
        # placeholders are accepted/overridden by qpdf and our parser, but the
        # installed MuPDF keeps them free and cannot render that synthetic
        # source. Sparse subsections avoid that reader interoperability issue.
        i = 0
        while i < size:
            if rows[i*13] == 2:
                i += 1
                continue
            end = i+1
            while end < size and rows[end*13] != 2:
                end += 1
            body.extend(f"{i} {end-i}\n".encode())
            for n in range(i, end):
                row = rows[n*13:(n+1)*13]
                assert row[0] in (0, 1)
                body.extend(f"{int.from_bytes(row[1:9], 'big'):010} {int.from_bytes(row[9:], 'big'):05} {'n' if row[0] else 'f'} \n".encode())
            i = end
        body.extend(b"trailer\n" + trailer.unparse(resolved=True))
        body.extend(f"\nstartxref\n{position}\n%%EOF\n".encode())
        output.write_bytes(body)


hybrid = OUT / "synthetic-hybrid.pdf"
hybrid_from(stream, hybrid)

sources = [(p, "") for p in (source, packed, mupdf, stream, hybrid)]
for name, mode, clear in [("r6-classic", "disable", False), ("r6-objstm", "generate", False),
                           ("r6-clear-metadata", "generate", True)]:
    output = OUT / (name + ".pdf")
    run(name+"-produce", "qpdf", f"--object-streams={mode}", "--encrypt", USER, OWNER, "256",
        *(["--cleartext-metadata"] if clear else []), "--", source, output)
    sources.append((output, USER))
native_r6 = OUT / "rzpdf-r6-objstm.pdf"
run("native-r6-produce", BIN / "v012_security_layout", source, native_r6, "objects", "set", "flate")
sources.append((native_r6, USER))
r6_hybrid = OUT / "synthetic-r6-hybrid.pdf"
hybrid_from(native_r6, r6_hybrid, USER)
sources.insert(0, (r6_hybrid, USER))

# Authenticated direct Encrypt identity is preserved, without allocating a new
# encryption object in the incremental tail.
direct = OUT / "synthetic-direct-encrypt-r6.pdf"
classic = OUT / "r6-classic.pdf"
report = run("direct-source-xref", "qpdf", f"--password={OWNER}", "--show-xref", classic)
offsets = {int(n): (int(g), int(o)) for n, g, o in re.findall(r"(\d+)/(\d+): uncompressed; offset = (\d+)", report)}
with pikepdf.open(classic, password=OWNER, attempt_recovery=False) as pdf:
    trailer = pikepdf.Dictionary(pdf.trailer)
    encryption = trailer.Encrypt.objgen[0]
    trailer.Encrypt = pikepdf.Dictionary(trailer.Encrypt)
    data = bytearray(classic.read_bytes())
    data.extend(b"\n")
    position = len(data)
    size = int(trailer.Size)
    data.extend(f"xref\n0 {size}\n0000000000 65535 f \n".encode())
    for n in range(1, size):
        if n in offsets and n != encryption:
            generation, offset = offsets[n]
            data.extend(f"{offset:010} {generation:05} n \n".encode())
        else:
            data.extend(b"0000000000 00001 f \n")
    data.extend(b"trailer\n" + trailer.unparse(resolved=True))
    data.extend(f"\nstartxref\n{position}\n%%EOF\n".encode())
    direct.write_bytes(data)
sources.append((direct, USER))

for original, password in sources:
    run(original.stem+"-source-check", "qpdf", *([f"--password={password}"] if password else []), "--check", original)
    reference = render(original.stem+"-reference", original, password)
    with pikepdf.open(original, password=password, attempt_recovery=False) as pdf:
        member = pdf.Root.RzMutation.objgen
        info = pdf.trailer.Info.objgen
        baseline = projection(pdf, {member, info})
        info_other = pikepdf.Dictionary(pdf.trailer.Info)
        del info_other["/Title"]
        info_other = info_other.unparse(resolved=True)
        ids = [bytes(v) for v in pdf.trailer.ID]
        encrypt = pdf.trailer.Encrypt.unparse(resolved=True) if password else None
        before_ids = {o.objgen for o in pdf.objects if o.objgen != (0, 0)}
    noop = OUT / (original.stem+"-noop.pdf")
    run(original.stem+"-noop", BIN / "v012_incremental", original, noop, "stream", "noop", "none", "0", password=password)
    assert noop.read_bytes() == original.read_bytes()
    for sequence in ("scs", "csc"):
        previous = original
        previous_ids = ids
        excluded = {member, info}
        for round_no, code in enumerate(sequence, 1):
            label = f"{original.stem}-{sequence}-{round_no}"
            title = f"private-{sequence}-{round_no}"
            output = OUT / (label+".pdf")
            role = OWNER if password and round_no == 2 else password
            run(label+"-native", BIN / "v012_incremental", previous, output,
                "stream" if code == "s" else "classic", title, *member, password=role)
            old, data = previous.read_bytes(), output.read_bytes()
            assert data.startswith(old)
            assert (data[last(data):].startswith(b"xref")) == (code == "c")
            run(label+"-qpdf", "qpdf", *([f"--password={password}"] if password else []), "--check", output)
            xref = run(label+"-xref", "qpdf", *([f"--password={password}"] if password else []), "--show-xref", output)
            assert re.search(rf"^{member[0]}/0: uncompressed; offset = (\d+)$", xref, re.M)
            with pikepdf.open(output, password=password, attempt_recovery=False) as pdf:
                assert int(pdf.trailer.Prev) == last(old)
                assert "/XRefStm" not in pdf.trailer
                assert str(pdf.docinfo.Title) == title and str(pdf.Root.RzMutation) == title
                current_info = pikepdf.Dictionary(pdf.trailer.Info)
                del current_info["/Title"]
                assert current_info.unparse(resolved=True) == info_other
                excluded.add(pdf.trailer.Info.objgen)
                assert pdf.Root.RzPrivate.objgen == pdf.Root.RzAlias.objgen == pdf.Root.RzPrivate.Self.objgen
                assert projection(pdf, excluded) == baseline
                if password:
                    assert pdf.trailer.Encrypt.unparse(resolved=True) == encrypt
                    next_ids = [bytes(v) for v in pdf.trailer.ID]
                    assert next_ids[0] == ids[0] and next_ids[1] != previous_ids[1]
                    previous_ids = next_ids
                    assert title.encode() not in data[len(old):]
                    assert not pikepdf.open(output, password=OWNER, attempt_recovery=False).get_warnings()
                    try:
                        pikepdf.open(output, password="wrong", attempt_recovery=False)
                    except pikepdf.PasswordError:
                        pass
                    else:
                        raise AssertionError("wrong password accepted")
                else:
                    assert [bytes(v) for v in pdf.trailer.ID] == ids
                if code == "s":
                    prefix = data[last(data):]
                    self_id = int(prefix.split()[0])
                    assert self_id >= int(pikepdf.open(previous, password=password, attempt_recovery=False).trailer.Size)
                    stream_obj = pdf.get_object((self_id, 0))
                    assert stream_obj.Type == pikepdf.Name.XRef
                    assert list(stream_obj.W) == [1, 8, 4]
                    assert "/Filter" not in stream_obj and "/XRefStm" not in stream_obj
                    index = list(stream_obj.Index)
                    assert len(index) % 2 == 0 and index[0] == 0
                    rows = stream_obj.read_raw_bytes()
                    assert len(rows) == int(stream_obj.Length) == sum(index[1::2])*13
                    at, represented = 0, {}
                    for start, count in zip(index[::2], index[1::2]):
                        for n in range(start, start+count):
                            row = rows[at:at+13]
                            assert row[0] in (0, 1)  # no newly compressed entries
                            represented[n] = (row[0], int.from_bytes(row[1:9], "big"), int.from_bytes(row[9:], "big"))
                            at += 13
                    assert represented[self_id] == (1, last(data), 0)
                    assert represented[member[0]][0] == 1 and represented[member[0]][2] == 0
                    assert represented[member[0]][1] >= len(old)
                    assert represented[0][0] == 0 and represented[0][2] == 65535
                now_ids = {o.objgen for o in pdf.objects if o.objgen != (0, 0)}
                new_objects = now_ids-before_ids
                for objgen in new_objects:
                    obj = pdf.get_object(objgen)
                    if structural(obj):
                        assert obj.Type == pikepdf.Name.XRef
                assert not pdf.get_warnings()
            folder = render(label+"-render", output, password)
            for page in (1, 2):
                with Image.open(reference/f"page-{page}.png") as a, Image.open(folder/f"page-{page}.png") as b:
                    assert a.size == b.size and ImageChops.difference(a.convert("RGB"), b.convert("RGB")).getbbox() is None
                pixels.append((label, page))
                pictures.append((label+f" p{page}", folder/f"page-{page}.png"))
            records.append(dict(case=label, bytes=len(data), prefix_bytes=len(old), prev=last(old), startxref=last(data),
                sha256=hashlib.sha256(data).hexdigest(), layout=code, protected=bool(password)))
            previous, before_ids = output, now_ids
        # The next sequence restarts at the original source.
        with pikepdf.open(original, password=password, attempt_recovery=False) as pdf:
            before_ids = {o.objgen for o in pdf.objects if o.objgen != (0, 0)}

signed = OUT / "signed-source.pdf"
signed.write_bytes(bytes.fromhex((ROOT/"pdf/tests/fixtures/v011/workflow-certified.pdf.hex").read_text()))
denied = OUT / "denied.pdf"
run("signed-denied", BIN/"v012_incremental", signed, denied, "stream", "noop", "none", "0", success=False)
assert not denied.exists()

for sheet_no, start in enumerate(range(0, len(pictures), 24), 1):
    selected = pictures[start:start+24]
    sheet = Image.new("RGB", (960, 6*170), "white")
    draw = ImageDraw.Draw(sheet)
    for i, (label, filename) in enumerate(selected):
        x, y = (i%4)*240, (i//4)*170
        with Image.open(filename) as image:
            image.thumbnail((234, 140))
            sheet.paste(image, (x, y))
        draw.text((x+2, y+142), label, fill="black")
    sheet.save(OUT/f"contact-{sheet_no}.png")

(OUT/"manifest.json").write_text(json.dumps(dict(cases=records, pixel_pairs=pixels, inputs=len(sources),
    noops=len(sources), gate="V12-05", license="Original synthetic fixtures; no third-party document content"), indent=2))
print(f"V12-05 cases={len(records)} pixel_pairs={len(pixels)} noops={len(sources)}")

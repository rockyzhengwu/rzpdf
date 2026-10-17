"""Stage C types and Unicode appearance interoperability, without Poppler."""
import argparse
import json
from pathlib import Path
import re
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]


def independent_fixture(path):
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(520, 520))
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1, BaseFont=Name.Helvetica,
                                            Encoding=Name.WinAnsiEncoding))
        resources = Dictionary(Font=Dictionary(Helv=font))
        root = pdf.make_indirect(Dictionary(T=String("external")))
        fields, annotations = Array(), Array()

        def appearance(width, height, text=b"q Q"):
            stream = pdf.make_stream(text)
            stream.Type, stream.Subtype = Name.XObject, Name.Form
            stream.BBox, stream.Resources = Array([0, 0, width, height]), resources
            return stream

        def field(partial, ft, flags=0, **values):
            obj = pdf.make_indirect(Dictionary(T=String(partial), FT=ft, Ff=flags, Parent=root, **values))
            fields.append(obj)
            return obj

        def widget(field, rect, on=None, **values):
            width, height = rect[2] - rect[0], rect[3] - rect[1]
            if on:
                off = appearance(width, height, f"0 G 1 1 {width-2} {height-2} re S".encode())
                checked = appearance(width, height, f"0 g 2 2 {width-4} {height-4} re f".encode())
                normal = Dictionary(Off=off)
                normal["/" + on] = checked
                ap = Dictionary(N=normal, R=normal, D=normal)
            else:
                ap = Dictionary(N=appearance(width, height))
            obj = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Widget, Parent=field,
                                               Rect=Array(rect), P=page.obj, F=4, AP=ap, **values))
            annotations.append(obj)
            return obj

        radio = field("radio", Name.Btn, 1 << 15, V=Name.Red)
        radio.Kids = Array([widget(radio, [30, 440, 54, 464], "Red", AS=Name.Red),
                            widget(radio, [90, 440, 114, 464], "Blue", AS=Name.Off)])
        check = field("check", Name.Btn, V=Name.Off)
        check.Kids = Array([widget(check, [150, 440, 174, 464], "Custom", AS=Name.Off)])
        options = Array([Array(["export-a", "Alpha"]), Array(["export-b", "Beta"]), Array(["export-c", "Gamma"])])
        choice = field("list", Name.Ch, 1 << 21, Opt=options, V=String("export-b"), I=Array([1]))
        choice.Kids = Array([widget(choice, [30, 300, 220, 400])])
        combo = field("combo", Name.Ch, 1 << 17, Opt=options, V=String("export-a"))
        combo.Kids = Array([widget(combo, [260, 350, 480, 390])])
        # DA and Q are inherited; the child must not need a local override.
        text = field("text", Name.Tx, 1 << 12, V=String("Original"))
        text.Kids = Array([widget(text, [30, 160, 480, 240])])
        push = field("push", Name.Btn, 1 << 16)
        push.Kids = Array([widget(push, [260, 280, 480, 320], MK=Dictionary(CA=String("Original button")))])
        root.Kids, root.DA = fields, String("/Helv 12 Tf .1 .2 .3 rg")
        page.obj.Annots = annotations
        pdf.Root.AcroForm = Dictionary(Fields=Array([root]), DR=resources, Q=2, NeedAppearances=False)
        pdf.save(path, min_version="1.7")


def field_map(pdf):
    fields = {}

    def walk(node, prefix):
        name = prefix + ("." if prefix else "") + str(node.get("/T", ""))
        kids = node.get("/Kids", [])
        if kids and "/T" in kids[0]:
            for child in kids:
                assert child.Parent.objgen == node.objgen
                walk(child, name)
        else:
            fields[name] = node

    for node in pdf.Root.AcroForm.Fields:
        walk(node, "")
    return fields


def widgets(field):
    return list(field.Kids) if "/Kids" in field else [field]


def check_buttons(field, expected):
    assert str(field.V) == expected
    flags = int(field.get("/Ff", 0))
    active = []
    for widget in widgets(field):
        assert str(widget.AS) in widget.AP.N.keys()
        assert widget.AP.N[widget.AS].Subtype == Name.Form
        if widget.AS != Name.Off:
            assert widget.AS == field.V
            active.append(widget)
    assert active or expected == "/Off"
    if flags & (1 << 15) and not flags & (1 << 25):
        assert len(active) <= 1


def check_structure(path, external):
    with pikepdf.open(path) as pdf:
        fields = field_map(pdf)
        if external:
            check_buttons(fields["external.radio"], "/Blue")
            check_buttons(fields["external.check"], "/Custom")
            listing, combo = fields["external.list"], fields["external.combo"]
            assert [str(v) for v in listing.V] == ["export-a", "export-c"] and list(listing.I) == [0, 2]
            assert str(combo.V) == "export-b"
            text, push = fields["external.text"], fields["external.push"]
            assert str(text.V) == "Updated\nsecond line"
            # /Q comes from the document-wide default, not a local field key.
            assert "/Q" not in text and "/Q" not in text.Parent
            x_positions = [float(operands[4]) for operands, operator in pikepdf.parse_content_stream(widgets(text)[0].AP.N) if str(operator) == "Tm"]
            assert len(x_positions) == 2 and min(x_positions) > 380
            assert str(widgets(push)[0].MK.CA) == "Updated button"
            for field in [fields["external.radio"], fields["external.check"]]:
                for widget in widgets(field):
                    # State changes must not discard independently authored AP art.
                    assert widget.AP.R == widget.AP.N and widget.AP.D == widget.AP.N
        else:
            check_buttons(fields["controls.check"], "/Approved")
            check_buttons(fields["controls.radio"], "/Blue")
            check_buttons(fields["controls.unison"], "/Same")
            check_buttons(fields["controls.exclusive"], "/Same")
            assert [str(v) for v in fields["unicode.exports"].Opt] == ["你好", "世界"]
            check_buttons(fields["unicode.exports"], "/1")
            listing, combo = fields["controls.list"], fields["controls.combo"]
            assert [str(v) for v in listing.V] == ["a", "c"] and list(listing.I) == [0, 2]
            assert str(combo.V) == "Custom option"
            text, push = fields["unicode.multiline"], fields["unicode.push"]
            assert str(text.V) == "世界\n你好"
            assert str(widgets(push)[0].MK.CA) == "你好！"
            # Verify font mappings and absolute alignment from the CID widths,
            # not merely that the AP happens to have a Tj operation.
            for alignment in range(3):
                field = fields[f"unicode.align{alignment}"]
                widget = widgets(field)[0]
                stream = widget.AP.N
                font = next(value for _, value in stream.Resources.Font.items())
                assert font.Subtype == Name.Type0 and font.Encoding == Name("/Identity-H")
                cid_font = font.DescendantFonts[0]
                assert len(cid_font.FontDescriptor.FontFile2.read_bytes()) > 1000
                cmap = font.ToUnicode.read_bytes().decode("ascii")
                mappings = {int(a, 16): bytes.fromhex(b).decode("utf-16-be") for a, b in re.findall(r"<([0-9A-Fa-f]{4})>\s+<([0-9A-Fa-f]{4,8})>", cmap)}
                codes, matrix, size = None, None, None
                for operands, operator in pikepdf.parse_content_stream(stream):
                    if str(operator) == "Tf":
                        size = float(operands[1])
                    elif str(operator) == "Tm":
                        matrix = [float(v) for v in operands]
                    elif str(operator) == "Tj":
                        codes = bytes(operands[0])
                ids = [int.from_bytes(codes[i:i+2], "big") for i in range(0, len(codes), 2)]
                assert "".join(mappings[cid] for cid in ids) == "你好，世界！"
                # This corpus uses the existing fixed-repertoire contiguous W.
                start, widths = int(cid_font.W[0]), cid_font.W[1]
                advance = sum(float(widths[cid - start]) for cid in ids) * size / 1000
                width = float(stream.BBox[2]) - float(stream.BBox[0])
                expected_x = [3, (width - advance) / 2, width - 3 - advance][alignment]
                assert abs(matrix[4] - expected_x) < 0.002, (alignment, matrix, advance, expected_x)
                assert abs(matrix[5] - 14) < 0.002
        for field in fields.values():
            if int(field.get("/Ff", 0)) & (1 << 16):
                assert "/V" not in field and "/DV" not in field
            for widget in widgets(field):
                assert widget.objgen in [a.objgen for page in pdf.pages for a in page.obj.get("/Annots", [])]
                if widget.objgen != field.objgen:
                    assert widget.Parent.objgen == field.objgen and "/V" not in widget
                assert widget.AP.N is not None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)

    def run(label, *args):
        result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError((label, result.returncode, result.stderr))

    independent_fixture(out / "external-input.pdf")
    run("qpdf-input", "qpdf", "--check", out / "external-input.pdf")
    rows = []
    for label, count in [("own", 3), ("external", 1)]:
        args = [ROOT / "target/debug/examples/v09_form_types", out / f"{label}.pdf"]
        if label == "external":
            args.append(out / "external-input.pdf")
        run(f"generate-{label}", *args)
        check_structure(out / f"{label}.pdf", label == "external")
        run(f"qpdf-{label}", "qpdf", "--check", out / f"{label}.pdf")
        run(f"skia-{label}", ROOT / "target/debug/examples/render_annotations", out / f"{label}.pdf", out / f"skia-{label}")
        run(f"mupdf-{label}", "mutool", "draw", "-q", "-r", "72", "-o", out / f"mupdf-{label}-%d.png", out / f"{label}.pdf")
        for page in range(1, count + 1):
            with Image.open(out / f"skia-{label}/page-{page}.png") as im:
                actual = im.convert("RGB")
            with Image.open(out / f"mupdf-{label}-{page}.png") as im:
                reference = im.convert("RGB")
            assert actual.size == reference.size
            diff = ImageChops.difference(actual, reference)
            diff.save(out / f"difference-{label}-{page}.png")
            mean = sum(ImageStat.Stat(diff).mean) / 3
            assert mean < 1.5, (label, page, mean)
            rows.append({"corpus": label, "page": page, "mean_channel_error": mean})
    (out / "report.json").write_text(json.dumps(rows, indent=2) + "\n")
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()

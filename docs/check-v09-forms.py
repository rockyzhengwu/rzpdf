"""Field tree interoperability gate; existing pikepdf, Pillow, qpdf and MuPDF."""
import argparse
import json
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]


def independent_fixture(destination):
    with pikepdf.Pdf.new() as pdf:
        pages = [pdf.add_blank_page(page_size=(595, 842)) for _ in range(2)]
        helv = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1, BaseFont=Name.Helvetica))
        resources = Dictionary(Font=Dictionary(Helv=helv))

        def appearance(width, height, content):
            stream = pdf.make_stream(content)
            stream.Type, stream.Subtype = Name.XObject, Name.Form
            stream.BBox, stream.Resources = Array([0, 0, width, height]), resources
            return stream

        person = pdf.make_indirect(Dictionary(T=String("person")))
        address = pdf.make_indirect(Dictionary(T=String("address"), Parent=person, FT=Name.Tx,
                                              V=String("Inherited"), MaxLen=20))
        city = pdf.make_indirect(Dictionary(T=String("city"), Parent=address, Custom=String("keep me")))
        accepted = pdf.make_indirect(Dictionary(T=String("accepted"), Parent=person, FT=Name.Btn, V=Name.Off))
        person.Kids, address.Kids = Array([address, accepted]), Array([city])
        city_widgets, check_widgets = Array(), Array()
        for i, page in enumerate(pages):
            text_rect = [20, 40, 180, 70] if i == 0 else [25, 150, 225, 190]
            check_rect = [20, 90, 44, 114] if i == 0 else [25, 220, 49, 244]
            text = appearance(text_rect[2] - text_rect[0], text_rect[3] - text_rect[1],
                              b"BT /Helv 12 Tf 3 4 Td (Inherited) Tj ET")
            widget = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Widget, Parent=city,
                                                  Rect=Array(text_rect), F=4, AP=Dictionary(N=text, R=text, D=text)))
            # /P is optional; only the second widget carries it.
            if i:
                widget.P = page.obj
            off = appearance(24, 24, b"0 G 1 1 22 22 re S")
            on = appearance(24, 24, b"0 G 1 1 22 22 re S 3 3 m 21 21 l S")
            check = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Widget, Parent=accepted,
                                                 P=page.obj, Rect=Array(check_rect), F=4, AS=Name.Off,
                                                 AP=Dictionary(N=Dictionary(Off=off, Yes=on))))
            city_widgets.append(widget)
            check_widgets.append(check)
            page.obj.Annots = Array([widget, check])
        city.Kids, accepted.Kids = city_widgets, check_widgets
        pdf.Root.AcroForm = Dictionary(Fields=Array([person]), DR=resources,
                                       DA=String("/Helv 12 Tf 0 g"), NeedAppearances=False)
        pdf.save(destination, min_version="1.7")


def check_structure(path, external):
    with pikepdf.open(path) as pdf:
        root = pdf.Root.AcroForm.Fields[0]
        assert str(root.T) == "customer"
        address, accepted = root.Kids
        city = address.Kids[0]
        assert str(city.T) == "city" and str(city.V) == "Suzhou"
        assert accepted.V == Name.Yes
        assert city.Parent.objgen == address.objgen and address.Parent.objgen == root.objgen
        assert accepted.Parent.objgen == root.objgen
        assert len(city.Kids) == len(accepted.Kids) == 2
        if external:
            assert str(city.Custom) == "keep me"
            assert str(address.V) == "Inherited" and address.MaxLen == 20
        for page, text, check in zip(pdf.pages, city.Kids, accepted.Kids):
            assert [a.objgen for a in page.obj.Annots] == [text.objgen, check.objgen]
            for widget, parent in [(text, city), (check, accepted)]:
                assert widget.Parent.objgen == parent.objgen
                assert "/V" not in widget and "/T" not in widget
                if "/P" in widget:
                    assert widget.P.objgen == page.obj.objgen
            assert text.AP.N.Subtype == Name.Form and text.AP.N.Resources.Font.Helv.BaseFont == Name.Helvetica
            shown = [bytes(operands[0]) for operands, operator in pikepdf.parse_content_stream(text.AP.N) if str(operator) == "Tj"]
            assert shown == [b"Suzhou"]
            if external:
                assert b"(Inherited)" in text.AP.R.read_bytes() and b"(Inherited)" in text.AP.D.read_bytes()
            assert check.AS == Name.Yes and set(check.AP.N.keys()) == {"/Off", "/Yes"}


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
    for label in ["own", "external"]:
        arguments = [ROOT / "target/debug/examples/v09_forms", out / f"{label}.pdf"]
        if label == "external":
            arguments.append(out / "external-input.pdf")
        run(f"fill-{label}", *arguments)
        run(f"qpdf-{label}", "qpdf", "--check", out / f"{label}.pdf")
        check_structure(out / f"{label}.pdf", label == "external")
        run(f"skia-{label}", ROOT / "target/debug/examples/render_annotations", out / f"{label}.pdf", out / f"skia-{label}")
        run(f"mupdf-{label}", "mutool", "draw", "-q", "-r", "72", "-o", out / f"mupdf-{label}-%d.png", out / f"{label}.pdf")
        for page in [1, 2]:
            with Image.open(out / f"skia-{label}/page-{page}.png") as im:
                actual = im.convert("RGB")
            with Image.open(out / f"mupdf-{label}-{page}.png") as im:
                reference = im.convert("RGB")
            assert actual.size == reference.size
            assert ImageChops.difference(actual, Image.new("RGB", actual.size, "white")).getbbox()
            difference = ImageChops.difference(actual, reference)
            difference.save(out / f"difference-{label}-{page}.png")
            mean = sum(ImageStat.Stat(difference).mean) / 3
            assert mean < 1.0, (label, page, mean)
            rows.append({"corpus": label, "page": page, "mean_channel_error": mean})
    (out / "report.json").write_text(json.dumps(rows, indent=2) + "\n")
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()

"""Generated annotation interoperability gate; existing pikepdf/Pillow/qpdf/MuPDF only."""
import argparse
import json
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]


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
        return result.stdout

    run("generate", ROOT / "target/debug/examples/v09_annotations", out / "annotations.pdf")
    run("qpdf-own", "qpdf", "--check", out / "annotations.pdf")
    with pikepdf.open(out / "annotations.pdf") as pdf:
        annotations = pdf.pages[0].obj.Annots
        expected = ["Square", "Circle", "Text", "Popup", "FreeText", "Line", "Polygon", "PolyLine", "Highlight", "Underline", "Squiggly", "StrikeOut", "Ink", "Stamp", "FileAttachment"]
        assert [str(a.Subtype)[1:] for a in annotations] == expected
        for a in annotations:
            assert a.P.objgen == pdf.pages[0].obj.objgen
            if a.Subtype != Name.Popup:
                assert a.AP.N.Subtype == Name.Form and a.AP.N.Resources is not None
        assert annotations[-1].FS.EF.F.read_bytes() == b"v0.9 annotation payload"
        assert annotations[2].Popup.objgen == annotations[3].objgen
        assert annotations[3].Parent.objgen == annotations[2].objgen
        assert annotations[12].IRT.objgen == annotations[2].objgen
        assert annotations[13].IRT.objgen == annotations[12].objgen
        assert "/RT" not in annotations[13]  # ordinary reply uses the default R
        assert str(annotations[4].Contents) == "Free text\nsecond line"
        assert pdf.pages[1].obj.Annots[-1].FT == Name.Tx
        # Constant annotation opacity is applied outside an isolated artwork.
        ink = pdf.pages[1].obj.Annots[2].AP.N
        assert ink.Resources.XObject.Art.Group.I
        assert float(ink.Resources.ExtGState.Opacity.ca) == 0.5
        assert pdf.pages[1].obj.Annots[1].AP.N.Resources.ExtGState.Opacity.BM == Name.Multiply

    # Independent AP matrices/local resources/state dictionaries, crop, rotation,
    # UserUnit, NoRotate and visibility. No downloaded or redistributed fixture.
    pdf = pikepdf.Pdf.new()
    fields = Array()
    for i, rotation in enumerate([0, 90, 180, 270]):
        page = pdf.add_blank_page(page_size=(240, 160))
        page.obj.CropBox = Array([10, 20, 210, 140])
        page.obj.Rotate = rotation
        page.obj.UserUnit = 2 if i == 2 else 1
        form = pdf.make_stream(b"0 0 1 rg -10 -20 20 60 re f 1 0 0 rg 10 -20 20 60 re f")
        form.Type, form.Subtype = Name.XObject, Name.Form
        form.BBox, form.Matrix = Array([-10, -20, 30, 40]), Array([0, 1, -1, 0, 4, 5])
        form.Resources = Dictionary()
        annotations = Array()
        upright = pdf.make_stream(b"0 0 1 rg 0 0 45 20 re f 1 0 0 rg 0 20 45 20 re f")
        upright.Type, upright.Subtype, upright.BBox, upright.Resources = Name.XObject, Name.Form, Array([0, 0, 45, 40]), Dictionary()
        for rect, flags, appearance in [([30, 40, 90, 90], 4, form), ([115, 45, 160, 85], 20, upright)]:
            annotations.append(pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square, Rect=Array(rect), P=page.obj, F=flags, AP=Dictionary(N=appearance))))
        on, off = pdf.make_stream(b"0 1 0 rg 0 0 20 20 re f"), pdf.make_stream(b"1 0 0 rg 0 0 20 20 re f")
        for state in [on, off]:
            state.Type, state.Subtype, state.BBox, state.Resources = Name.XObject, Name.Form, Array([0, 0, 20, 20]), Dictionary()
        widget = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Widget, FT=Name.Btn, T=String(f"check-{i}"), V=Name.On, AS=Name.On, F=4, Rect=Array([170, 100, 190, 120]), P=page.obj, AP=Dictionary(N=Dictionary(On=on, Off=off))))
        annotations.append(widget)
        fields.append(widget)
        # Hidden malformed AP must not be selected, decoded or interpreted.
        annotations.append(pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square, Rect=Array([30, 100, 60, 120]), P=page.obj, F=2, AP=Dictionary(N=42))))
        page.obj.Annots = annotations
    pdf.Root.AcroForm = Dictionary(Fields=fields, NeedAppearances=False)
    pdf.save(out / "external.pdf", min_version="1.7")
    pdf.close()
    run("qpdf-external", "qpdf", "--check", out / "external.pdf")
    # MuPDF 1.27 misplaces NoRotate for the nonzero BBox + rotated Matrix
    # combination. Preserve that original diagnostic instead of raising the
    # tolerance. Both APs describe exactly the same unrotated red/blue artwork.
    with pikepdf.open(out / "external.pdf") as diagnostic:
        for page in diagnostic.pages:
            page.obj.Annots[1].AP.N = page.obj.Annots[0].AP.N
        diagnostic.save(out / "external-nontrivial-no-rotate.pdf")

    rows = []
    for label, count in [("annotations", 2), ("external", 4)]:
        run(f"skia-{label}", ROOT / "target/debug/examples/render_annotations", out / f"{label}.pdf", out / f"skia-{label}")
        run(f"mupdf-{label}", "mutool", "draw", "-q", "-r", "72", "-o", out / f"mupdf-{label}-%d.png", out / f"{label}.pdf")
        for page in range(1, count + 1):
            with Image.open(out / f"skia-{label}/page-{page}.png") as im:
                actual = im.convert("RGB")
            with Image.open(out / f"mupdf-{label}-{page}.png") as im:
                reference = im.convert("RGB")
            assert actual.size == reference.size, (label, page, actual.size, reference.size)
            difference = ImageChops.difference(actual, reference)
            difference.save(out / f"difference-{label}-{page}.png")
            mean = sum(ImageStat.Stat(difference).mean) / 3
            assert mean <= 3.0, (label, page, mean)
            rows.append({"corpus": label, "page": page, "mean_channel_error": mean})
            if label == "annotations" and page == 2:
                for point, expected in [((50, 50), (128, 0, 128)), ((150, 55), (0, 0, 0)), ((50, 150), (128, 128, 128)), ((30, 170), (128, 128, 128))]:
                    for reader, image in [("Skia", actual), ("MuPDF", reference)]:
                        value = image.getpixel(point)
                        assert max(abs(a - b) for a, b in zip(value, expected)) <= 2, (reader, point, value, expected)
    diagnostic_name = "external-nontrivial-no-rotate"
    run("qpdf-diagnostic", "qpdf", "--check", out / f"{diagnostic_name}.pdf")
    run("skia-diagnostic", ROOT / "target/debug/examples/render_annotations", out / f"{diagnostic_name}.pdf", out / "skia-diagnostic")
    run("mupdf-diagnostic", "mutool", "draw", "-q", "-r", "72", "-o", out / "mupdf-diagnostic-%d.png", out / f"{diagnostic_name}.pdf")
    diagnostics = []
    for page in range(1, 5):
        actual = Image.open(out / f"skia-diagnostic/page-{page}.png").convert("RGB")
        canonical = Image.open(out / f"skia-external/page-{page}.png").convert("RGB")
        equivalent_error = sum(ImageStat.Stat(ImageChops.difference(actual, canonical)).mean) / 3
        assert equivalent_error <= 0.01, ("equivalent AP invariant", page, equivalent_error)
        mupdf = Image.open(out / f"mupdf-diagnostic-{page}.png").convert("RGB")
        discrepancy = sum(ImageStat.Stat(ImageChops.difference(actual, mupdf)).mean) / 3
        diagnostics.append({"page": page, "equivalent_AP_error": equivalent_error, "original_MuPDF_discrepancy": discrepancy})
    (out / "report.json").write_text(json.dumps({"pages": rows, "nontrivial_no_rotate_diagnostic": diagnostics}, indent=2) + "\n")
    print("PASS", out, flush=True)


if __name__ == "__main__":
    main()

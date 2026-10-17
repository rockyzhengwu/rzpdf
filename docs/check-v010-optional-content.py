"""Independent optional-content production, roundtrip, text and pixel gate.

Uses existing pikepdf/qpdf/MuPDF/Pillow; no Poppler or package installation.
The fixture is generated here, without input from the engine under test.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
BLACK = b"q 0 g 0 0 80 40 re W n 0 0 160 40 re f Q\n"
RED = b"1 0 0 rg 10 100 40 40 re f\n"
BLUE = b"0 0 1 rg 70 100 40 40 re f\n"
ORANGE = b"1 0.5 0 rg 10 60 40 20 re f\n"
MAGENTA = b"1 0 1 rg 70 60 20 20 re f\n"
GREEN = b"0 1 0 rg 130 100 30 30 re f\n"
TEXT = b"BT /F1 12 Tf 10 100 Td /OC /B BDC /Span << /ActualText (SECRET) >> BDC (X) Tj EMC EMC (Z) Tj ET\n"


def produce(path):
    with pikepdf.Pdf.new() as pdf:
        first, second = [pdf.add_blank_page(page_size=(200, 200)).obj for _ in range(2)]
        a = pdf.make_indirect(Dictionary(Type=Name.OCG, Name=String("Visible")))
        b = pdf.make_indirect(Dictionary(Type=Name.OCG, Name=String("Hidden"),
            Usage=Dictionary(Print=Dictionary(PrintState=Name.ON))))
        member = pdf.make_indirect(Dictionary(Type=Name.OCMD,
            OCGs=Array([a, b]), P=Name.AllOff,
            VE=Array([Name.And, a, Array([Name.Not, b])])))
        auto = Array([Dictionary(Event=Name.Print, Category=Array([Name.Print]), OCGs=Array([b]))])
        pdf.Root.OCProperties = Dictionary(OCGs=Array([a, b]),
            D=Dictionary(Name=String("Default"), OFF=Array([b]), AS=auto,
                         Order=Array([a, b]), PrivateUI=String("preserved")),
            Configs=Array([Dictionary(Name=String("Swap"), BaseState=Name.OFF,
                                      ON=Array([b]), AS=auto)]))
        # Private UI data is preserved at OCProperties, not an unknown rule in D.
        del pdf.Root.OCProperties.D.PrivateUI
        pdf.Root.OCProperties.PrivateUI = String("preserved")
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1,
            BaseFont=Name.Courier, Encoding=Name.WinAnsiEncoding))
        form = pdf.make_stream(MAGENTA)
        form.Type, form.Subtype, form.BBox = Name.XObject, Name.Form, Array([0, 0, 200, 200])
        form.Resources, form.OC = Dictionary(), b
        form.Group = Dictionary(S=Name.Transparency, I=True)
        ap = pdf.make_stream(b"0 1 0 rg 0 0 30 30 re f\n")
        ap.Type, ap.Subtype, ap.BBox = Name.XObject, Name.Form, Array([0, 0, 30, 30])
        ap.Resources = Dictionary()
        annot = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square,
            Rect=Array([130, 100, 160, 130]), P=first, F=4, OC=b, AP=Dictionary(N=ap)))
        first.Annots = Array([annot])
        for page in [first, second]:
            page.Resources = Dictionary(Font=Dictionary(F1=font),
                Properties=Dictionary(A=a, B=b, M=member), XObject=Dictionary(Fm=form))
        first.Contents = pdf.make_stream(
            b"q /OC /B BDC 0 0 80 40 re W n EMC 0 g 0 0 160 40 re f Q\n"
            + b"/OC /A BDC " + RED + b"EMC\n/OC /B BDC " + BLUE
            + b"EMC\n/OC /M BDC " + ORANGE + b"EMC\n/Fm Do\n")
        second.Contents = pdf.make_stream(TEXT)
        pdf.save(path, min_version="1.7")


def expected(path, a, b):
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(200, 200)).obj
        page.Contents = pdf.make_stream(BLACK + (RED if a else b"") + (BLUE if b else b"")
            + (ORANGE if a and not b else b"") + (MAGENTA + GREEN if b else b""))
        pdf.save(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = out / "source.pdf"
    produce(source)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()

    def run(label, *args, success=True):
        completed = subprocess.run([str(a) for a in args], cwd=ROOT,
                                   capture_output=True, text=True)
        (out / f"{label}.log").write_text(completed.stdout + completed.stderr)
        if (completed.returncode == 0) != success:
            raise RuntimeError((label, completed.returncode, completed.stdout, completed.stderr))
        return completed.stdout

    inspect = ROOT / "target/debug/examples/inspect_optional_content"
    render = ROOT / "target/debug/examples/render_optional_content"
    run("qpdf-source", "qpdf", "--check", source)
    comparisons, images = [], []
    for label, config, intent, a, b, text in [
        ("default", "default", "view", True, False, "Z"),
        ("swap", "0", "view", False, True, "SECRETZ"),
        ("print", "default", "print", True, True, "SECRETZ"),
    ]:
        rewritten = out / f"{label}.pdf"
        log = run(f"inspect-{label}", inspect, source, rewritten, config, intent)
        assert 'groups=2 configurations=1' in log
        assert f'page=2 text="{text}"' in log
        assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
        run(f"qpdf-{label}", "qpdf", "--check", rewritten)
        with pikepdf.open(source) as original, pikepdf.open(rewritten) as saved:
            assert str(saved.Root.OCProperties.PrivateUI) == "preserved"
            assert str(saved.Root.OCProperties.D.Name) == "Default"
            assert len(saved.Root.OCProperties.Configs) == 1
            assert saved.Root.OCProperties.D.OFF[0].objgen == saved.Root.OCProperties.OCGs[1].objgen
            for i in range(2):
                assert saved.pages[i].Contents.read_bytes() == original.pages[i].Contents.read_bytes()
            assert saved.pages[0].Resources.XObject.Fm.OC.objgen == saved.Root.OCProperties.OCGs[1].objgen
            assert saved.pages[0].Annots[0].OC.objgen == saved.Root.OCProperties.OCGs[1].objgen
        run(f"reinspect-{label}", inspect, rewritten, out / f"reopened-{label}.pdf", config, intent)
        pngdir = out / f"skia-{label}"
        run(f"render-{label}", render, source, pngdir, config, intent)
        expected_pdf, expected_png = out / f"expected-{label}.pdf", out / f"expected-{label}.png"
        expected(expected_pdf, a, b)
        run(f"mupdf-{label}", "mutool", "draw", "-q", "-r", "72", "-o", expected_png, expected_pdf)
        actual = Image.open(pngdir / "page-1.png").convert("RGB")
        reference = Image.open(expected_png).convert("RGB")
        difference = ImageChops.difference(actual, reference)
        changed = sum(pixel != (0, 0, 0) for pixel in difference.get_flattened_data())
        maximum_delta = max(high for low, high in difference.getextrema())
        # Integer-aligned geometry must match. Skia rounds 0.5 to 128 while
        # MuPDF rounds it to 127; allow one RGB quantization level, not a
        # displacement or an incorrect visibility decision.
        assert actual.size == reference.size == (200, 200)
        assert maximum_delta <= 1, (label, changed, maximum_delta, difference.getbbox())
        actual_mask = actual.point(lambda value: 255 if value == 255 else 0)
        reference_mask = reference.point(lambda value: 255 if value == 255 else 0)
        assert ImageChops.difference(actual_mask, reference_mask).getbbox() is None
        comparisons.append(dict(configuration=label, changed_pixels=changed,
                                maximum_channel_delta=maximum_delta, geometry_difference_pixels=0))
        images.extend([reference, actual])
    comparison = Image.new("RGB", (400, 600), "white")
    for index, im in enumerate(images):
        comparison.paste(im, ((index % 2) * 200, (index // 2) * 200))
    comparison.save(out / "comparison.png")
    existing_digest = hashlib.sha256((out / "default.pdf").read_bytes()).hexdigest()
    run("existing-output", inspect, source, out / "default.pdf", "default", "view", success=False)
    assert hashlib.sha256((out / "default.pdf").read_bytes()).hexdigest() == existing_digest
    # qpdf-valid semantic negatives must not create an output PDF.
    for label in ["unknown-policy", "cyclic-ve", "unknown-usage", "unknown-config"]:
        bad = out / f"{label}.pdf"
        with pikepdf.open(source) as pdf:
            if label == "unknown-policy":
                member = pdf.pages[0].Resources.Properties.M
                del member.VE
                member.P = Name.Future
            elif label == "cyclic-ve":
                expression = pdf.make_indirect(Array([Name.Not, None]))
                expression[1] = expression
                pdf.pages[0].Resources.Properties.M.VE = expression
            elif label == "unknown-usage":
                pdf.Root.OCProperties.D.AS = Array([Dictionary(Event=Name.View,
                    Category=Array([Name.Zoom]), OCGs=pdf.Root.OCProperties.OCGs)])
                pdf.Root.OCProperties.OCGs[0].Usage = Dictionary(Zoom=Dictionary(min=1))
            else:
                pdf.Root.OCProperties.D.FutureRule = True
            pdf.save(bad)
        run(f"qpdf-{label}", "qpdf", "--check", bad)
        rejected = out / f"rejected-{label}.pdf"
        run(label, inspect, bad, rejected, "default", "view", success=False)
        assert not rejected.exists()
    (out / "report.json").write_text(json.dumps(dict(source_sha256=digest,
        comparisons=comparisons, negative_cases=4), indent=2) + "\n")
    print("optional content: roundtrip/text/configuration/3 pixel comparisons/4 refusals passed")


if __name__ == "__main__":
    main()

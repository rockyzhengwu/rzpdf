"""Independent page relationship and unchanged-appearance gate; no Poppler."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]


def independent_fixture(path):
    spec = importlib.util.spec_from_file_location("forms_gate", ROOT / "docs/check-v09-forms.py")
    forms = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(forms)
    forms.independent_fixture(path)
    with pikepdf.open(path, allow_overwriting_input=True) as pdf:
        first, second = [p.obj for p in pdf.pages]
        destination = pdf.make_indirect(Dictionary(D=pdf.make_indirect(Array([first, Name.Fit])), Custom=String("destination extension")))
        pdf.Root.Names = Dictionary(Dests=Dictionary(Names=Array([String("start"), destination])))
        root = pdf.make_indirect(Dictionary(Type=Name.Outlines, Count=2))
        item = pdf.make_indirect(Dictionary(Title=String("Start"), Parent=root, A=Dictionary(S=Name.GoTo, D=String("start"), Custom=String("action extension")), Count=1, C=Array([0, 0, 1]), F=2))
        child = pdf.make_indirect(Dictionary(Title=String("Next"), Parent=item, Dest=Array([second, Name.FitH, 200])))
        root.First = root.Last = item
        item.First = item.Last = child
        pdf.Root.Outlines = root
        pdf.Root.PageLabels = Dictionary(Nums=Array([0, Dictionary(S=Name.r, P=String("s-"), St=3)]))
        second.Annots.append(pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Link, P=second, Rect=Array([20, 270, 130, 295]), A=Dictionary(S=Name.GoTo, D=String("start")))))

        def appearance():
            ap = pdf.make_stream(b"1 0 0 RG 2 w 1 1 78 48 re S")
            ap.Type, ap.Subtype, ap.BBox, ap.Resources = Name.XObject, Name.Form, Array([0, 0, 80, 50]), Dictionary()
            return ap

        square = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square, P=first, Rect=Array([50, 300, 130, 350]), F=4, AP=Dictionary(N=appearance()), Custom=String("preserved")))
        popup = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Popup, P=first, Rect=Array([150, 300, 250, 350]), Parent=square, Open=False))
        square.Popup = popup
        reply = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Text, P=first, Rect=Array([270, 300, 350, 350]), IRT=square, Name=Name.Note, F=4, AP=Dictionary(N=appearance())))
        payload = pdf.make_stream(b"independently authored attachment")
        filespec = pdf.make_indirect(Dictionary(Type=Name.Filespec, F=String("sample.txt"), EF=Dictionary(F=payload)))
        attachment = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.FileAttachment, P=first, Rect=Array([370, 300, 450, 350]), Name=Name.PushPin, FS=filespec, F=4, AP=Dictionary(N=appearance())))
        first.Annots.extend([square, popup, reply, attachment])
        pdf.save(path)


def check_structure(path, external):
    with pikepdf.open(path) as pdf:
        assert len(pdf.pages) == 3
        first, blank, last = [p.obj for p in pdf.pages]
        assert len(blank.get("/Annots", [])) == 0
        key, target = pdf.Root.Names.Dests.Names
        assert str(key) == "imported_start"
        if external:
            assert str(target.Custom) == "destination extension"
            target = target.D
        assert target[0].objgen == first.objgen
        outline = pdf.Root.Outlines.First
        destination = outline.Dest if "/Dest" in outline else outline.A.D
        assert destination[0].objgen == first.objgen
        assert outline.First.Dest[0].objgen == last.objgen
        if external:
            assert list(outline.C) == [0, 0, 1] and outline.F == 2
            assert str(outline.A.Custom) == "action extension"
        all_widgets = {}
        for page in pdf.pages:
            records = list(page.obj.get("/Annots", []))
            ids = {a.objgen: a for a in records}
            for a in records:
                assert a.P.objgen == page.obj.objgen
                if a.Subtype == Name.Widget:
                    assert a.objgen not in all_widgets
                    all_widgets[a.objgen] = a
                if "/Popup" in a:
                    assert a.Popup.objgen in ids and a.Popup.Parent.objgen == a.objgen
                if "/IRT" in a:
                    assert a.IRT.objgen in ids
                if a.Subtype == Name.Link:
                    target = a.Dest if "/Dest" in a else a.A.D
                    assert target[0].objgen == first.objgen
                if a.Subtype == Name.FileAttachment:
                    assert a.FS.EF.F.read_bytes() == b"independently authored attachment"
                if external and a.Subtype == Name.Square:
                    assert str(a.Custom) == "preserved"
        seen_widgets, text_fields = set(), []

        def walk(field, parent=None, prefix="", value=None):
            assert field.get("/Parent", None) is None if parent is None else field.Parent.objgen == parent.objgen
            full = ".".join(filter(None, [prefix, str(field.get("/T", ""))]))
            value = field.get("/V", value)
            kids = list(field.get("/Kids", []))
            widgets = ([field] if field.get("/Subtype", None) == Name.Widget else
                       [k for k in kids if k.get("/Subtype", None) == Name.Widget and "/T" not in k])
            for w in widgets:
                assert w.objgen in all_widgets
                seen_widgets.add(w.objgen)
                if w.objgen != field.objgen:
                    assert w.Parent.objgen == field.objgen
                assert "/AP" in w
            if widgets and isinstance(value, pikepdf.String):
                text_fields.append((full, len(widgets)))
                assert str(value) == ("Inherited" if external else "Shanghai")
            for child in kids:
                if child not in widgets:
                    walk(child, field, full, value)

        for field in pdf.Root.AcroForm.Fields:
            walk(field)
        assert seen_widgets == set(all_widgets)
        assert len(text_fields) == 2 and all(count == 1 for _, count in text_fields), text_fields
        assert {name.split(".")[0] for name, _ in text_fields} == {"imported_person", "copy_imported_person"}
        return {"fields": text_fields, "widgets": len(seen_widgets)}


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
    rows = []
    for corpus in ["own", "external"]:
        base = out / corpus
        arguments = [ROOT / "target/debug/examples/v09_page_relationships", base]
        if corpus == "external":
            arguments.append(out / "external-input.pdf")
        run(f"transfer-{corpus}", *arguments)
        for stage in ["source", "imported", "copied", "final", "refilled"]:
            run(f"qpdf-{corpus}-{stage}", "qpdf", "--check", base / f"{stage}.pdf")
        structure = check_structure(base / "final.pdf", corpus == "external")
        for stage in ["source", "final"]:
            run(f"skia-{corpus}-{stage}", ROOT / "target/debug/examples/render_annotations", base / f"{stage}.pdf", base / f"skia-{stage}")
        run(f"mupdf-{corpus}", "mutool", "draw", "-q", "-r", "72", "-o", base / "mupdf-%d.png", base / "final.pdf")
        for original, final in [(1, 1), (2, 3)]:
            with Image.open(base / f"skia-source/page-{original}.png") as im:
                before = im.convert("RGB")
            with Image.open(base / f"skia-final/page-{final}.png") as im:
                after = im.convert("RGB")
            assert ImageChops.difference(before, after).getbbox() is None, (corpus, original, "changed appearance")
        for page in range(1, 4):
            with Image.open(base / f"skia-final/page-{page}.png") as im:
                actual = im.convert("RGB")
            with Image.open(base / f"mupdf-{page}.png") as im:
                reference = im.convert("RGB")
            assert actual.size == reference.size
            diff = ImageChops.difference(actual, reference)
            diff.save(base / f"difference-{page}.png")
            mean = sum(ImageStat.Stat(diff).mean) / 3
            assert mean < 1.0, (corpus, page, mean)
            rows.append({"corpus": corpus, "page": page, "mean_channel_error": mean, **structure})
    (out / "report.json").write_text(json.dumps(rows, indent=2) + "\n")
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()

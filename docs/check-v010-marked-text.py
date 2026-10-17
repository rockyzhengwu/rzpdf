"""Independent logical text and declared-order gate; no rendering/conformance claim."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import pikepdf
from pikepdf import Array, Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]
PAGE = (b"BT /F1 12 Tf 10 180 Td /P << /MCID 0 >> BDC (A) Tj EMC "
        b"/Span /Props BDC (B) Tj (C) Tj EMC /Artifact BMC (H) Tj EMC ET /Fm Do")
FORM = b"/Span << /MCID 0 /ActualText (F) >> BDC BT /F1 12 Tf (D) Tj ET EMC"
SECOND = b"BT /F1 12 Tf /P << /MCID 0 >> BDC (Z) Tj EMC ET"


def produce(path):
    with pikepdf.Pdf.new() as pdf:
        page, second = [pdf.add_blank_page(page_size=(200, 200)).obj for _ in range(2)]
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1,
                                           BaseFont=Name.Courier, Encoding=Name.WinAnsiEncoding))
        form = pdf.make_stream(FORM)
        form.Type, form.Subtype = Name.XObject, Name.Form
        form.BBox, form.StructParents = Array([0, 0, 100, 100]), 1
        form.Resources = Dictionary(Font=Dictionary(F1=font))
        page.Resources = Dictionary(Font=Dictionary(F1=font), XObject=Dictionary(Fm=form),
                                    Properties=Dictionary(Props=Dictionary(MCID=1, ActualText=String("你好"))))
        second.Resources = Dictionary(Font=Dictionary(F1=font))
        root = pdf.make_indirect(Dictionary(Type=Name.StructTreeRoot))
        group = pdf.make_indirect(Dictionary(S=Name.Document, P=root))
        first = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=page, K=0,
                                             ActualText=String("element-A"), Custom=String("keep")))
        replacement = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=page, K=1))
        form_element = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=page,
                                  K=Dictionary(Type=Name.MCR, Stm=form, MCID=0)))
        last = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=second, K=0))
        root.K, group.K = group, Array([form_element, replacement, last, first])
        root.ParentTree = Dictionary(Nums=Array([0, Array([first, replacement]),
                                                1, Array([form_element]), 2, Array([last])]))
        root.ParentTreeNextKey = 3
        page.StructParents, second.StructParents = 0, 2
        page.Contents, second.Contents = pdf.make_stream(PAGE), pdf.make_stream(SECOND)
        pdf.Root.StructTreeRoot = root
        pdf.Root.MarkInfo = Dictionary(Marked=True)
        pdf.save(path, min_version="1.7")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source, rewritten = out / "independent.pdf", out / "rewritten.pdf"
    produce(source)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    binary = ROOT / "target/debug/examples/extract_marked_text"

    def run(label, *args, success=True):
        result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(result.stdout + result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError((label, result.returncode, result.stdout, result.stderr))
        return result.stdout

    run("qpdf-source", "qpdf", "--check", source)
    report = run("extract", binary, source, rewritten)
    assert 'page[0] logical="A你好F" raw="ABCHD"' in report
    assert 'page[1] logical="Z" raw="Z"' in report
    assert 'page[0] logical_units=3 replacements=2' in report
    assert 'structure="F你好Zelement-A" unassociated=0 object_refs=0' in report
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    run("qpdf-rewritten", "qpdf", "--check", rewritten)
    assert run("reextract", binary, rewritten) == report
    with pikepdf.open(rewritten) as pdf:
        root = pdf.Root.StructTreeRoot
        form_element, replacement, last, first = root.K.K
        page, second = [p.obj for p in pdf.pages]
        assert page.Contents.read_bytes() == PAGE and second.Contents.read_bytes() == SECOND
        assert page.Resources.XObject.Fm.read_bytes() == FORM
        assert form_element.K.Stm.objgen == page.Resources.XObject.Fm.objgen
        assert str(first.ActualText) == "element-A" and str(first.Custom) == "keep"
        assert str(page.Resources.Properties.Props.ActualText) == "你好"
        nums = root.ParentTree.Nums
        assert nums[1][0].objgen == first.objgen and nums[1][1].objgen == replacement.objgen
        assert nums[5][0].objgen == last.objgen
    # MuPDF supplies an independent marked ActualText extraction observation;
    # whitespace/layout and structure-element replacement are not equated.
    run("mutool-text", "mutool", "draw", "-q", "-F", "txt", "-o", out / "mutool.txt", source)
    observed = (out / "mutool.txt").read_text()
    assert "你好" in observed and "F" in observed
    for label, mutate in (
        ("wrong-parent", lambda p: p.Root.StructTreeRoot.ParentTree.Nums[1].__setitem__(0, None)),
        ("missing-properties", lambda p: p.pages[0].obj.Resources.__delitem__("/Properties")),
        ("cross-owner-replacement", lambda p: setattr(p.pages[0].obj, "Contents",
            p.make_stream(b"/Span << /ActualText (atomic) >> BDC " + PAGE + b" EMC"))),
    ):
        bad, rejected = out / f"{label}.pdf", out / f"{label}-must-not-exist.pdf"
        with pikepdf.open(source) as pdf:
            mutate(pdf)
            pdf.save(bad)
        run(f"{label}-qpdf", "qpdf", "--check", bad)
        run(label, binary, bad, rejected, success=False)
        assert not rejected.exists()
    print("PASS independent ActualText, Properties, Artifact, page/Form scopes and declared cross-page K order")
    print("PASS strict rewrite, original content/metadata/ParentTree preservation, three semantic rejections")
    print("PASS MuPDF marked ActualText observation; NOT a layout-order or PDF/UA conformance comparison")


if __name__ == "__main__":
    main()

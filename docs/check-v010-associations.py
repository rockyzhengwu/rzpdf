"""Independent producer and roundtrip checks for PDF 1.x structure associations."""
import argparse
import hashlib
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source, rewritten = out / "independent.pdf", out / "rewritten.pdf"
    page_content = b"/P << /MCID 0 >> BDC EMC /F Do /F Do"
    second_content = b"/P << /MCID 2 >> BDC EMC /F Do"
    form_content = b"/Span /Props BDC EMC"
    with pikepdf.Pdf.new() as pdf:
        page, second = [pdf.add_blank_page(page_size=(200, 200)).obj for _ in range(2)]
        root = pdf.make_indirect(Dictionary(Type=Name.StructTreeRoot))
        group = pdf.make_indirect(Dictionary(S=Name.Document, P=root))
        paragraph = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=page,
                                                 ActualText=String("你好"), Custom=String("keep")))
        other = pdf.make_indirect(Dictionary(S=Name.P, P=group, Pg=second, K=2))
        figure = pdf.make_indirect(Dictionary(S=Name.Figure, P=group, Pg=page))
        form = pdf.make_stream(form_content)
        form.Type, form.Subtype = Name.XObject, Name.Form
        form.BBox, form.StructParents = Array([0, 0, 10, 10]), 1
        # No Form Resources: /Props resolves in each invoking page's scope.
        for current in (page, second):
            current.Resources = Dictionary(XObject=Dictionary(F=form),
                                           Properties=Dictionary(Props=Dictionary(MCID=0)))
        annot = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square, P=page,
                                             Rect=Array([10, 10, 40, 40]), StructParent=3))
        page.Annots, page.StructParents, second.StructParents = Array([annot]), 0, 2
        page.Contents, second.Contents = pdf.make_stream(page_content), pdf.make_stream(second_content)
        paragraph.K = Array([0, Dictionary(Type=Name.MCR, Stm=form, MCID=0)])
        figure.K = Dictionary(Type=Name.OBJR, Obj=annot)
        group.K, root.K = Array([paragraph, other, figure]), group
        root.ParentTree = Dictionary(Nums=Array([0, Array([paragraph]), 1, Array([paragraph]),
                                                2, Array([None, None, other]), 3, figure]))
        root.ParentTreeNextKey = 4
        pdf.Root.StructTreeRoot = root
        pdf.Root.MarkInfo = Dictionary(Marked=True)
        pdf.save(source, min_version="1.7")
    original_hash = hashlib.sha256(source.read_bytes()).hexdigest()

    def run(label, *args, success=True):
        result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(result.stdout + result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError((label, result.returncode, result.stdout, result.stderr))
        return result.stdout + result.stderr

    binary = ROOT / "target/debug/examples/validate_structure_associations"
    run("qpdf-source", "qpdf", "--check", source)
    report = run("validate", binary, source, rewritten)
    assert "associations=4 marked_content=3 object_references=1" in report
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original_hash
    run("qpdf-rewritten", "qpdf", "--check", rewritten)
    assert run("revalidate", binary, rewritten) == report
    with pikepdf.open(rewritten) as pdf:
        page, second = [p.obj for p in pdf.pages]
        paragraph, other, figure = pdf.Root.StructTreeRoot.K.K
        form = page.Resources.XObject.F
        assert paragraph.K[1].Stm.objgen == form.objgen == second.Resources.XObject.F.objgen
        assert str(paragraph.Custom) == "keep" and str(paragraph.ActualText) == "你好"
        assert page.Contents.read_bytes() == page_content
        assert second.Contents.read_bytes() == second_content
        assert form.read_bytes() == form_content
        assert figure.K.Obj.objgen == page.Annots[0].objgen
        nums = pdf.Root.StructTreeRoot.ParentTree.Nums
        assert nums[1][0].objgen == nums[3][0].objgen == paragraph.objgen
        assert nums[5][0] is None and nums[5][1] is None and nums[5][2].objgen == other.objgen
        assert nums[7].objgen == figure.objgen
    # Syntactically valid PDFs with deliberately incoherent semantic ownership.
    for label, mutate in (
        ("wrong-owner", lambda p: p.Root.StructTreeRoot.ParentTree.Nums[1].__setitem__(0, p.Root.StructTreeRoot.K)),
        ("missing-mcid", lambda p: setattr(p.pages[0].obj, "Contents", p.make_stream(b"/F Do"))),
    ):
        bad, rejected = out / f"{label}.pdf", out / f"{label}-must-not-exist.pdf"
        with pikepdf.open(source) as pdf:
            mutate(pdf)
            pdf.save(bad)
        run(f"qpdf-{label}", "qpdf", "--check", bad)
        run(label, binary, bad, rejected, success=False)
        assert not rejected.exists()
    print("PASS independent page/Form domains, shared Form, Properties, MCR, OBJR, ParentTree null slots")
    print("PASS strict roundtrip, pikepdf pointer/content preservation, qpdf checks, two semantic rejections")
    print("NOT VERIFIED: attributes/namespaces, ActualText extraction, appearance-stream MCR, layers, PDF/UA")


if __name__ == "__main__":
    main()

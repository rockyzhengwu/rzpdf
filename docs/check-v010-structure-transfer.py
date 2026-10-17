"""Independent page migration, shared Form isolation and structure edit gate."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import pikepdf
from pikepdf import Array, Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]
FIRST = b"BT /F1 12 Tf 10 180 Td /Span << /MCID 0 >> BDC (A) Tj EMC ET /Fm Do\n"
SECOND = b"BT /F1 12 Tf 10 180 Td /Span << /MCID 0 >> BDC (B) Tj EMC ET /Fm Do\n"
FORM = b"/Span << /MCID 0 /ActualText (F) >> BDC BT /F1 12 Tf (D) Tj ET EMC\n"
APPEARANCE = b"0 0 10 10 re S\n"


def produce(path):
    with pikepdf.Pdf.new() as pdf:
        first, second = [pdf.add_blank_page(page_size=(200, 200)).obj for _ in range(2)]
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1,
                                           BaseFont=Name.Courier, Encoding=Name.WinAnsiEncoding))
        form = pdf.make_stream(FORM)
        form.Type, form.Subtype = Name.XObject, Name.Form
        form.BBox, form.StructParents = Array([0, 0, 100, 100]), 2
        form.Resources = Dictionary(Font=Dictionary(F1=font))
        for page, content, key in [(first, FIRST, 0), (second, SECOND, 1)]:
            page.Resources = Dictionary(Font=Dictionary(F1=font), XObject=Dictionary(Fm=form))
            page.Contents = pdf.make_stream(content)
            page.StructParents = key
        appearance = pdf.make_stream(APPEARANCE)
        appearance.Type, appearance.Subtype = Name.XObject, Name.Form
        appearance.BBox, appearance.Resources = Array([0, 0, 10, 10]), Dictionary()
        annot = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square,
                                            Rect=Array([0, 0, 10, 10]), P=first,
                                            AP=Dictionary(N=appearance), StructParent=3))
        first.Annots = Array([annot])
        root = pdf.make_indirect(Dictionary(Type=Name.StructTreeRoot))
        group = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.Document, P=root))
        a = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.CustomParagraph,
            P=group, Pg=first, T=String("标题"), C=Name.Body,
            K=Array([0, Dictionary(Type=Name.OBJR, Pg=first, Obj=annot)])))
        f = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.P, P=group, Pg=first,
            K=Dictionary(Type=Name.MCR, Stm=form, MCID=0)))
        b = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.P, P=group, Pg=second, K=0))
        root.K, group.K = group, Array([a, f, b])
        root.RoleMap = Dictionary(CustomParagraph=Name.P)
        root.ClassMap = Dictionary(Body=Dictionary(O=Name.Layout, Placement=Name.Block))
        root.ParentTree = Dictionary(Nums=Array([0, Array([a]), 1, Array([b]),
                                                2, Array([f]), 3, a]))
        root.ParentTreeNextKey = 4
        pdf.Root.StructTreeRoot, pdf.Root.MarkInfo = root, Dictionary(Marked=True)
        pdf.save(path, min_version="1.7")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source, result = out / "source.pdf", out / "transferred.pdf"
    produce(source)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    binary = ROOT / "target/debug/examples/transfer_structure"

    def run(label, *args, success=True):
        completed = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(completed.stdout + completed.stderr)
        if (completed.returncode == 0) != success:
            raise RuntimeError((label, completed.returncode, completed.stdout, completed.stderr))
        return completed.stdout

    run("qpdf-source", "qpdf", "--check", source)
    assert 'structure="FBZF" pages=2' in run("transfer", binary, source, result)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    result_digest = hashlib.sha256(result.read_bytes()).hexdigest()
    run("existing-output", binary, source, result, success=False)
    assert hashlib.sha256(result.read_bytes()).hexdigest() == result_digest
    run("qpdf-result", "qpdf", "--check", result)
    with pikepdf.open(result) as pdf:
        root = pdf.Root.StructTreeRoot
        assert len(pdf.pages) == 2 and len(root.K) == 2
        assert str(root.RoleMap.CustomParagraph) == "/P"
        assert str(root.ClassMap.Body.Placement) == "/Block"
        keep, copied = root.K
        f, b = keep.K
        z, cloned_f = copied.K
        p0, p1 = [p.obj for p in pdf.pages]
        assert str(z.T) == "标题" and str(z.C) == "/Body"
        shows = [bytes(operands[0]) for operands, operator in pikepdf.parse_content_stream(p0)
                 if str(operator) == "Tj"]
        assert shows == [b"Z"]
        assert b"(B) Tj" in p1.Contents.read_bytes()
        form0, form1 = p0.Resources.XObject.Fm, p1.Resources.XObject.Fm
        assert form0.objgen != form1.objgen
        assert form0.read_bytes() == FORM and form1.read_bytes() == FORM
        assert f.K[0].Stm.objgen == form1.objgen and f.K[0].Pg.objgen == p1.objgen
        assert cloned_f.K[0].Stm.objgen == form0.objgen and cloned_f.K[0].Pg.objgen == p0.objgen
        assert len(p0.Annots) == 1 and "/Annots" not in p1
        annot = p0.Annots[0]
        assert annot.P.objgen == p0.objgen and annot.AP.N.read_bytes() == APPEARANCE
        assert z.K[1].Obj.objgen == annot.objgen and z.K[1].Pg.objgen == p0.objgen
        owners = [p0.StructParents, p1.StructParents, form0.StructParents, form1.StructParents,
                  annot.StructParent]
        assert len(set(int(k) for k in owners)) == 5
        nums = root.ParentTree.Nums
        reverse = {int(nums[i]): nums[i + 1] for i in range(0, len(nums), 2)}
        for owner, element in [(p0, z), (p1, b), (form0, cloned_f), (form1, f)]:
            assert reverse[int(owner.StructParents)][0].objgen == element.objgen
        assert reverse[int(annot.StructParent)].objgen == z.objgen
        assert root.ParentTreeNextKey > max(reverse)
        for group in root.K:
            assert group.P.objgen == root.objgen
            for node in group.K:
                assert node.P.objgen == group.objgen
    run("mupdf-text", "mutool", "draw", "-q", "-F", "txt", "-o", out / "mutool.txt", result)
    observed = (out / "mutool.txt").read_text()
    assert "Z" in observed and "B" in observed and "F" in observed
    for label, mutate in (
        ("bad-parenttree", lambda p: p.Root.StructTreeRoot.ParentTree.Nums[1].__setitem__(0, None)),
        ("split-replacement", lambda p: setattr(p.Root.StructTreeRoot.K, "ActualText", String("whole"))),
        ("unknown-idtree", lambda p: setattr(p.Root.StructTreeRoot, "IDTree", Dictionary(Names=Array([])))),
        ("optional-config", lambda p: setattr(p.Root, "OCProperties", Dictionary(OCGs=Array([]), D=Dictionary()))),
    ):
        bad, rejected = out / f"{label}.pdf", out / f"{label}-must-not-exist.pdf"
        with pikepdf.open(source) as pdf:
            mutate(pdf)
            pdf.save(bad)
        run(f"{label}-qpdf", "qpdf", "--check", bad)
        run(label, binary, bad, rejected, success=False)
        assert not rejected.exists()
    print("PASS copy/reorder/delete/body edit/import and strict reopen with independent producer")
    print("PASS shared Form isolation/rebinding, annotation OBJR/AP, maps and unique ParentTree keys")
    print("PASS source/output preservation, semantic rejection and MuPDF text; no pixel/PDF-UA claim")


if __name__ == "__main__":
    main()

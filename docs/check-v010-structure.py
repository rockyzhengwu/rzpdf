"""Hierarchy inspection only: no ParentTree/content/Tagged PDF conformance claim."""
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
    source = out / "independent.pdf"
    rewritten = out / "rewritten.pdf"
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(200, 200)).obj
        root = pdf.make_indirect(Dictionary(Type=Name.StructTreeRoot))
        group = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.Document, P=root))
        # /Type is optional on a structure element in PDF 1.x.
        paragraph = pdf.make_indirect(Dictionary(S=Name.CustomParagraph, P=group, Pg=page,
                                                 ActualText=String("你好"), Custom=String("keep")))
        figure = pdf.make_indirect(Dictionary(Type=Name.StructElem, S=Name.Figure, P=group,
                                              Pg=page, Alt=String("example")))
        annot = pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Square, P=page,
                                             Rect=Array([10, 10, 40, 40]), StructParent=1))
        page.Annots, page.StructParents = Array([annot]), 0
        content = b"/P << /MCID 0 >> BDC EMC /P << /MCID 1 >> BDC EMC"
        page.Contents = pdf.make_stream(content)
        paragraph.K = Array([0, Dictionary(Type=Name.MCR, MCID=1, Pg=page)])
        figure.K = Dictionary(Type=Name.OBJR, Obj=annot, Pg=page)
        group.K, root.K = Array([paragraph, figure]), group
        root.RoleMap = Dictionary(CustomParagraph=Name.P)
        root.ParentTree = Dictionary(Nums=Array([0, Array([paragraph, paragraph]), 1, figure]))
        pdf.Root.StructTreeRoot = root
        pdf.save(source, min_version="1.7")
    original_hash = hashlib.sha256(source.read_bytes()).hexdigest()

    def run(label, *args):
        process = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(process.stdout + process.stderr)
        if process.returncode:
            raise RuntimeError((label, process.returncode, process.stderr))
        return process.stdout

    run("qpdf-source", "qpdf", "--check", source)
    report = run("inspect", ROOT / "target/debug/examples/inspect_structure", source, rewritten)
    assert "elements=3 content_validation=not_performed parent_tree_validation=not_performed" in report
    assert 'role=CustomParagraph mapped_role=Some("P") raw_content=2 actual_text=Some("你好")' in report
    assert 'role=Figure mapped_role=Some("Figure") raw_content=1' in report
    assert report.index("role=Document") < report.index("role=CustomParagraph") < report.index("role=Figure")
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original_hash
    run("qpdf-rewritten", "qpdf", "--check", rewritten)
    assert run("reinspect", ROOT / "target/debug/examples/inspect_structure", rewritten) == report
    with pikepdf.open(rewritten) as pdf:
        group = pdf.Root.StructTreeRoot.K
        paragraph, figure = group.K
        assert paragraph.P.objgen == group.objgen and figure.P.objgen == group.objgen
        assert str(paragraph.Custom) == "keep" and str(paragraph.ActualText) == "你好"
        assert pdf.pages[0].obj.Contents.read_bytes() == content
        assert figure.K.Obj.objgen == pdf.pages[0].obj.Annots[0].objgen
        assert pdf.Root.StructTreeRoot.ParentTree.Nums[1][0].objgen == paragraph.objgen
    print("PASS hierarchy inspection, independent producer, strict rewrite, unchanged content")
    print("NOT VERIFIED: ParentTree/content coherence, ActualText extraction, layers or Tagged PDF conformance")


if __name__ == "__main__":
    main()

"""Independent namespace and local-attribute checks; not Tagged/PDF-UA conformance."""
import argparse
import hashlib
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]
CONTENT = b"/P << /MCID 0 >> BDC EMC /P << /MCID 1 >> BDC EMC"


def produce(path, modern):
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(200, 200)).obj
        root = pdf.make_indirect(Dictionary(Type=Name.StructTreeRoot))
        group = pdf.make_indirect(Dictionary(S=Name.Document, P=root))
        paragraph = pdf.make_indirect(Dictionary(S=Name.X, P=group, Pg=page, K=0, R=3,
                                                 C=Array([Name.First, Name.Second, Name.Absent]),
                                                 A=Array([Dictionary(O=Name.Layout, TextAlign=Name.Center), 3]),
                                                 ActualText=String("你好"), Custom=String("keep")))
        other = pdf.make_indirect(Dictionary(S=Name.X, P=group, Pg=page, K=1,
                                             C=Array([Name.First, Name.Second])))
        root.ClassMap = Dictionary(
            First=Dictionary(O=Name.Layout, TextAlign=Name.Start, WritingMode=Name.LrTb,
                             Private=Array([String("unknown"), 7])),
            Second=Dictionary(O=Name.Layout, TextAlign=Name.End))
        root.RoleMap = Dictionary(X=Name.P if not modern else Name.Wrong)
        if modern:
            standard = pdf.make_indirect(Dictionary(Type=Name.Namespace, NS=String("http://iso.org/pdf2/ssn")))
            custom = pdf.make_indirect(Dictionary(NS=String("urn:example:custom"),
                RoleMapNS=Dictionary(X=Array([Name.P, standard])),
                Schema=Dictionary(FS=Name.URL, F=String("https://invalid.example/schema"))))
            alternate = pdf.make_indirect(Dictionary(NS=String("urn:example:alternate"),
                RoleMapNS=Dictionary(X=Name.Span)))
            root.Namespaces = Array([standard, custom, alternate])
            group.NS, paragraph.NS, other.NS = standard, custom, alternate
            paragraph.A.append(Dictionary(O=Name.NSO, NS=custom, Score=42))
        root.K, group.K = group, Array([paragraph, other])
        root.ParentTree = Dictionary(Nums=Array([0, Array([paragraph, other])]))
        page.StructParents, page.Contents = 0, pdf.make_stream(CONTENT)
        pdf.Root.StructTreeRoot = root
        pdf.save(path, min_version="2.0" if modern else "1.7")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    binary = ROOT / "target/debug/examples/inspect_structure_semantics"
    associations = ROOT / "target/debug/examples/validate_structure_associations"

    def run(label, *args, success=True):
        result = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(result.stdout + result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError((label, result.returncode, result.stdout, result.stderr))
        return result.stdout

    for modern in (False, True):
        label = "pdf2" if modern else "pdf1"
        source, rewritten = out / f"{label}.pdf", out / f"{label}-rewritten.pdf"
        produce(source, modern)
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        run(f"{label}-qpdf-source", "qpdf", "--check", source)
        report = run(f"{label}-inspect", binary, source, rewritten)
        assert f"elements=3 namespaces={3 if modern else 0} classes=2" in report
        assert "Layout.TextAlign=/Center" in report and "Layout.WritingMode=/LrTb" in report
        assert f"Layout.TextAlign={'/End' if modern else 'ambiguous:2'}" in report
        if modern:
            assert 'role=X endpoint=P namespace="http://iso.org/pdf2/ssn"' in report
            assert 'role=X endpoint=Span namespace="http://iso.org/pdf/ssn"' in report
        assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
        run(f"{label}-qpdf-rewritten", "qpdf", "--check", rewritten)
        assert run(f"{label}-reinspect", binary, rewritten) == report
        assert "associations=2 marked_content=2 object_references=0" in run(f"{label}-associations", associations, rewritten)
        with pikepdf.open(rewritten) as pdf:
            root = pdf.Root.StructTreeRoot
            paragraph, other = root.K.K
            assert paragraph.P.objgen == other.P.objgen == root.K.objgen
            assert str(paragraph.Custom) == "keep" and str(paragraph.ActualText) == "你好"
            assert str(root.ClassMap.First.Private[0]) == "unknown"
            assert pdf.pages[0].obj.Contents.read_bytes() == CONTENT
            assert paragraph.A[1] == 3 and paragraph.R == 3
            if modern:
                assert paragraph.NS.RoleMapNS.X[1].objgen == root.K.NS.objgen
                assert paragraph.A[2].NS.objgen == paragraph.NS.objgen
                assert paragraph.A[2].Score == 42
                assert str(paragraph.NS.Schema.F) == "https://invalid.example/schema"
                assert len(root.Namespaces) == 3
    source = out / "pdf2.pdf"
    mutations = (
        ("namespace-cycle", lambda p: setattr(p.Root.StructTreeRoot.K.K[0].NS, "RoleMapNS",
            Dictionary(X=Array([Name.X, p.Root.StructTreeRoot.K.K[0].NS])))),
        ("unlisted-namespace", lambda p: setattr(p.Root.StructTreeRoot, "Namespaces",
            Array([p.Root.StructTreeRoot.K.NS]))),
        ("future-revision", lambda p: p.Root.StructTreeRoot.K.K[0].A.__setitem__(1, 4)),
    )
    for label, mutate in mutations:
        bad, rejected = out / f"{label}.pdf", out / f"{label}-must-not-exist.pdf"
        with pikepdf.open(source) as pdf:
            mutate(pdf)
            pdf.save(bad)
        run(f"{label}-qpdf", "qpdf", "--check", bad)
        run(label, binary, bad, rejected, success=False)
        assert not rejected.exists()
    print("PASS independent PDF 1.7/PDF 2.0 namespace identity, mappings, class/direct precedence, revisions")
    print("PASS strict rewrite, qpdf/pikepdf preservation, MCID/ParentTree integration, three semantic rejections")
    print("NOT VERIFIED: standard-role vocabularies, layout inheritance/defaults, opaque payload semantics, PDF/UA")


if __name__ == "__main__":
    main()

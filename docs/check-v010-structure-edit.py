"""Independent explicit structure writing, ownership and source preservation gate."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import pikepdf
from pikepdf import Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]
SOURCE = b"BT /F1 12 Tf 10 180 Td (A) Tj % keep comment\n(B) Tj ET\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source, result = out / "source.pdf", out / "structured.pdf"
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(200, 200)).obj
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1,
                                          BaseFont=Name.Courier, Encoding=Name.WinAnsiEncoding))
        page.Resources = Dictionary(Font=Dictionary(F1=font))
        page.Contents = pdf.make_stream(SOURCE)
        pdf.Root.Custom = String("preserve catalog metadata")
        pdf.save(source, min_version="1.7")
    digest = hashlib.sha256(source.read_bytes()).hexdigest()

    def run(label, *args, success=True):
        completed = subprocess.run([str(a) for a in args], cwd=ROOT, capture_output=True, text=True)
        (out / f"{label}.log").write_text(completed.stdout + completed.stderr)
        if (completed.returncode == 0) != success:
            raise RuntimeError((label, completed.returncode, completed.stdout, completed.stderr))
        return completed.stdout

    binary = ROOT / "target/debug/examples/edit_structure"
    run("qpdf-source", "qpdf", "--check", source)
    assert 'structure="你好A" raw="AB" elements=3' in run("edit", binary, source, result)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    result_digest = hashlib.sha256(result.read_bytes()).hexdigest()
    run("existing-output", binary, source, result, success=False)
    assert hashlib.sha256(result.read_bytes()).hexdigest() == result_digest
    run("qpdf-result", "qpdf", "--check", result)
    with pikepdf.open(result) as pdf:
        page = pdf.pages[0].obj
        root = pdf.Root.StructTreeRoot
        group = root.K[0]
        second, first = group.K
        assert group.P.objgen == root.objgen
        assert first.P.objgen == group.objgen and second.P.objgen == group.objgen
        assert str(first.S) == "/CustomParagraph" and str(root.RoleMap.CustomParagraph) == "/P"
        assert str(first.T) == "标题" and str(first.Lang) == "zh-CN"
        assert str(first.A[0].O) == "/Layout" and str(first.A[0].Placement) == "/Block"
        assert first.K[0].MCID == 0 and second.K[0].MCID == 1
        assert first.K[0].Pg.objgen == page.objgen and second.K[0].Pg.objgen == page.objgen
        nums = root.ParentTree.Nums
        assert nums[0] == page.StructParents and root.ParentTreeNextKey > nums[0]
        assert nums[1][0].objgen == first.objgen and nums[1][1].objgen == second.objgen
        assert str(pdf.Root.Custom) == "preserve catalog metadata"
        assert str(page.Resources.Font.F1.BaseFont) == "/Courier"
        decoded = page.Contents.read_bytes()
        assert b"% keep comment" in decoded and b"(A) Tj" in decoded and b"(B) Tj" in decoded
        instructions = list(pikepdf.parse_content_stream(page))
        stack, mcids = [], []
        for operands, operator in instructions:
            if str(operator) == "BDC":
                stack.append(operands[1])
                mcids.append(int(operands[1].MCID))
            elif str(operator) == "EMC":
                stack.pop()
        assert not stack and mcids == [0, 1]
        replacements = [str(operands[1].ActualText) for operands, op in instructions
                        if str(op) == "BDC" and "/ActualText" in operands[1]]
        assert replacements == ["你好"]
    run("mupdf-text", "mutool", "draw", "-q", "-F", "txt", "-o", out / "mutool.txt", result)
    observed = (out / "mutool.txt").read_text()
    assert "A" in observed and "你好" in observed
    for label, mutate in (
        ("existing-idtree", lambda p: setattr(p.Root, "StructTreeRoot", p.make_indirect(Dictionary(
            Type=Name.StructTreeRoot, IDTree=Dictionary(Names=pikepdf.Array([])))))),
        ("unsupported-mark", lambda p: setattr(p.pages[0].obj, "Contents", p.make_stream(
            b"/OC << >> BDC " + SOURCE + b" EMC\n"))),
    ):
        bad, rejected = out / f"{label}.pdf", out / f"{label}-must-not-exist.pdf"
        with pikepdf.open(source) as pdf:
            mutate(pdf)
            pdf.save(bad)
        run(f"{label}-qpdf", "qpdf", "--check", bad)
        run(label, binary, bad, rejected, success=False)
        assert not rejected.exists()
    print("PASS independent structure CRUD, RoleMap, attributes, MCIDs/ParentTree and strict reopen")
    print("PASS lexical text/comments, catalog/resources, source hash and rejection before output")
    print("PASS MuPDF marked ActualText observation; no PDF/UA or pixel conformance claim")


if __name__ == "__main__":
    main()

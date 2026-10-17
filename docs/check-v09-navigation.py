#!/usr/bin/env python3
"""Independent navigation gate. Requires existing pikepdf, Pillow, qpdf, mutool.

Run from repository root. All generated files stay in the supplied output folder.
The corpus is generated from scratch; no third-party PDF or font is redistributed.
"""
import argparse
from pathlib import Path
import subprocess

import pikepdf
from pikepdf import Array, Dictionary, Name, String
from PIL import Image


def run(*args):
    subprocess.run([str(x) for x in args], check=True)


def rust(example, *args):
    run("cargo", "run", "--locked", "-j2", "-p", "pdf", "--example", example, "--", *args)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    output = parser.parse_args().output
    output.mkdir(parents=True, exist_ok=False)
    own, external, edited = [output / name for name in ["navigation.pdf", "external.pdf", "external-edited.pdf"]]
    rust("v09_navigation", own)
    run("qpdf", "--check", own)
    with pikepdf.open(own) as pdf:
        assert len(pdf.pages) == 3
        assert pdf.Root.Names.Dests.Kids  # enough entries to force a balanced tree
        destinations = {}

        def walk(node):
            if "/Kids" in node:
                for kid in node.Kids:
                    yield from walk(kid)
            else:
                values = node.Names
                yield from ((str(values[i]), values[i + 1]) for i in range(0, len(values), 2))

        destinations.update(walk(pdf.Root.Names.Dests))
        assert len(destinations) == 71 and "旧目标" not in destinations
        dest = destinations["目标"]
        assert dest[0].objgen == pdf.pages[2].obj.objgen
        assert list(dest)[1:] == [Name.XYZ, 25, 180, None]
        root = pdf.Root.Outlines
        assert root.Count == 2
        assert str(root.First.Title) == "章节"
        assert str(root.First.Dest) == "目标"
        child = root.First.First
        assert child.Count == -1 and child.Parent.objgen == root.First.objgen
        assert child.First.Parent.objgen == child.objgen
        assert str(child.First.Dest) == "目标"
        links = pdf.pages[1].obj.Annots
        assert len(links) == 3 and str(links[0].Dest) == "目标"
        assert links[1].Dest[0].objgen == pdf.pages[2].obj.objgen
        assert str(links[2].A.URI) == "https://example.org/"
        nums = pdf.Root.PageLabels.Nums
        assert [(int(nums[i]), int(nums[i + 1].St)) for i in range(0, len(nums), 2)] == [(0, 3), (1, 1)]

    # Raster confirms that reordering did not change or lose page contents.
    run("mutool", "draw", "-q", "-r", "72", "-o", output / "page-%d.png", own)
    for index, color in enumerate([(0, 0, 255), (255, 0, 0), (0, 255, 0)], 1):
        with Image.open(output / f"page-{index}.png") as image:
            assert image.convert("RGB").getpixel((100, 100)) == color

    # Independently produced nested name/number trees and GoTo actions.
    pdf = pikepdf.Pdf.new()
    pages = [pdf.add_blank_page(page_size=(200, 200)) for _ in range(2)]
    first = pdf.make_indirect(Dictionary(Names=Array([String("alpha"), Array([pages[1].obj, Name.XYZ, None, 150, 1.25])]), Limits=Array([String("alpha"), String("alpha")])))
    second = pdf.make_indirect(Dictionary(Names=Array([String("beta"), Array([pages[0].obj, Name.FitBV, None])]), Limits=Array([String("beta"), String("beta")])))
    pdf.Root.Names = Dictionary(Dests=Dictionary(Kids=Array([first, second])))
    labels = []
    for index, style, prefix, start in [(0, Name.r, "pre-", 4), (1, Name.A, "章-", 27)]:
        labels.append(pdf.make_indirect(Dictionary(Nums=Array([index, Dictionary(S=style, P=String(prefix), St=start)]), Limits=Array([index, index]))))
    pdf.Root.PageLabels = Dictionary(Kids=Array(labels))
    outline_root = pdf.make_indirect(Dictionary(Type=Name.Outlines, Count=2))
    parent = pdf.make_indirect(Dictionary(Title=String("Independent parent"), Parent=outline_root, Dest=Array([pages[0].obj, Name.Fit]), Count=1))
    child = pdf.make_indirect(Dictionary(Title=String("独立子项"), Parent=parent, A=Dictionary(S=Name.GoTo, D=String("alpha"))))
    parent.First = parent.Last = child
    outline_root.First = outline_root.Last = parent
    pdf.Root.Outlines = outline_root
    pages[0].obj.Annots = Array([pdf.make_indirect(Dictionary(Type=Name.Annot, Subtype=Name.Link, P=pages[0].obj, Rect=Array([10, 10, 100, 30]), A=Dictionary(S=Name.GoTo, D=String("alpha"))))])
    pdf.save(external, min_version="1.7")
    pdf.close()
    run("qpdf", "--check", external)
    rust("v09_navigation_inspect", external, edited)
    run("qpdf", "--check", edited)
    with pikepdf.open(edited) as pdf:
        assert str(pdf.pages[0].obj.Annots[0].A.D) == "renamed"
        first = pdf.Root.Outlines.First
        assert str(first.A.D) == "renamed"
        assert str(first.Next.Title) == "Independent parent"
        assert "/First" not in first.Next and first.Parent.objgen == pdf.Root.Outlines.objgen
    print("v0.9 navigation gate: PASS (structure, independent read/edit, 3 raster pages)")


if __name__ == "__main__":
    main()

#!/usr/bin/env bash
# Validate the two-page, one-level Form edit example with independent tools.
set -euo pipefail
if [[ $# -ne 2 ]]; then
    echo 'Usage: bash docs/check-v02-form-example.sh EDITED.pdf NEW_EVIDENCE_DIR' >&2
    exit 2
fi
task_pdf=$1
task_evidence=$2
for task_tool in qpdf mutool python3; do
    command -v "$task_tool" >/dev/null || { echo "Missing tool: $task_tool" >&2; exit 2; }
done
mkdir "$task_evidence"
qpdf --check "$task_pdf"
[[ $(qpdf --show-npages "$task_pdf") == 2 ]]
mutool draw -q -F trace "$task_pdf" | python3 -c '
import sys
import xml.etree.ElementTree as ET

root = ET.fromstring(sys.stdin.read())
pages = root.findall("page")
if len(pages) != 2:
    raise ValueError("expected two pages")
for page, label in zip(pages, ["CDAB", "ABAB"]):
    if "".join(g.get("unicode", "") for g in page.findall(".//fill_text//g")) != label:
        raise ValueError("Form invocation text mismatch")
    if page.get("mediabox") != "0 0 230 110":
        raise ValueError("unexpected page size")
    images = page.findall(".//fill_image")
    paths = page.findall(".//stroke_path")
    if len(images) != 2 or len(paths) != 2:
        raise ValueError("Form draw count mismatch")
    if [float(image.get("transform").split()[4]) for image in images] != [20., 130.]:
        raise ValueError("image invocation placement changed")
    for path, right in zip(paths, [70 if label == "CDAB" else 60, 60]):
        points = [(node.tag, node.get("x"), node.get("y")) for node in path if node.tag in ("moveto", "lineto")]
        if points != [("moveto", "20", "20"), ("lineto", str(right), "20"),
                      ("lineto", str(right), "40"), ("lineto", "20", "40")]:
            raise ValueError("Form path geometry mismatch")
'
mutool draw -q -r 72 -F ppm -o "$task_evidence/form-%d.ppm" "$task_pdf"
mutool draw -q -r 96 -o "$task_evidence/form-%d.png" "$task_pdf"
python3 - "$task_evidence/form-1.ppm" "$task_evidence/form-2.ppm" <<'PY'
import sys
from pathlib import Path

def sample(path, x, y):
    with Path(path).open("rb") as stream:
        if stream.readline() != b"P6\n":
            raise ValueError("expected P6 PPM")
        width, height = map(int, stream.readline().split())
        if stream.readline() != b"255\n" or (width, height) != (230, 110):
            raise ValueError("unexpected pixel format or page size")
        stream.seek((y * width + x) * 3, 1)
        return tuple(stream.read(3))

edited, original = sys.argv[1:]
for path, first in [(edited, (0, 0, 255)), (original, (255, 0, 0))]:
    if sample(path, 40, 80) != first or sample(path, 150, 80) != (255, 0, 0):
        raise ValueError("selected/shared Form image colors mismatch")
PY
echo 'One-level Form edit: text, image pixels, path, repeated call and shared page passed.'

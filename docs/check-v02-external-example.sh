#!/usr/bin/env bash
# Check the controlled MuPDF fixture after edit_external_v02. Read-only for PDFs.
set -euo pipefail
if [[ $# -ne 3 ]]; then
    echo 'Usage: bash docs/check-v02-external-example.sh SOURCE.pdf EDITED.pdf NEW_EVIDENCE_DIR' >&2
    exit 2
fi
task_source=$1
task_edited=$2
task_evidence=$3
for task_tool in qpdf mutool jq python3 cmp; do
    command -v "$task_tool" >/dev/null || { echo "Missing tool: $task_tool" >&2; exit 2; }
done
mkdir "$task_evidence"
qpdf --check "$task_source"
qpdf --check "$task_edited"
qpdf --json --json-key=qpdf "$task_edited" | jq -e '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    deref($objects.trailer.value["/Root"]).value["/Pages"] as $tree |
    deref($tree).value["/Kids"][2] as $edited_page |
    [deref($edited_page).value["/Resources"]["/XObject"][] | deref(.) | .stream.dict |
        select(.["/Subtype"] == "/Image" and .["/SMask"] != null)] | length == 1
' >/dev/null
task_text=$(mutool draw -q -F txt "$task_edited" | tr -d '\f' | sed '/^[[:space:]]*$/d')
[[ "$task_text" == $'External producer - page 1\nExternal producer - page 2\nEdited producer - page 1' ]] || {
    echo 'Edited text or page order mismatch' >&2
    exit 1
}
mutool draw -q -F trace "$task_edited" | python3 -c '
import math
import sys
import xml.etree.ElementTree as ET

root = ET.fromstring(sys.stdin.read())
pages = root.findall("page")
if len(pages) != 3:
    raise ValueError("expected three pages")
expected = ["External producer - page 1", "External producer - page 2", "Edited producer - page 1"]
for page, label in zip(pages, expected):
    text = "".join(g.get("unicode", "") for g in page.findall(".//fill_text//g"))
    if text != label:
        raise ValueError("trace text mismatch")
if pages[0].get("mediabox") != pages[2].get("mediabox"):
    raise ValueError("edited page box changed")
images = pages[2].findall(".//fill_image")
if len(images) != 1 or images[0].get("width") != "2" or images[0].get("height") != "1":
    raise ValueError("replacement image missing")
transform = [float(value) for value in images[0].get("transform", "").split()]
if len(transform) != 6 or not all(math.isclose(a, b, abs_tol=0.02) for a, b in zip(transform, [80, 0, 0, 40, 120, 90])):
    raise ValueError("image display position mismatch")
paths = pages[2].findall(".//stroke_path")
if len(paths) != 1:
    raise ValueError("edited path missing")
points = [(tag.tag, tag.get("x"), tag.get("y")) for tag in paths[0] if tag.tag in ("moveto", "lineto")]
if points != [("moveto", "120", "30"), ("lineto", "200", "30"), ("lineto", "200", "70"), ("lineto", "120", "70")]:
    raise ValueError("rectangle geometry mismatch")
'
mutool draw -q -r 72 -o "$task_evidence/source-%d.png" "$task_source"
mutool draw -q -r 72 -o "$task_evidence/edited-%d.png" "$task_edited"
mutool draw -q -r 72 -F ppm -o "$task_evidence/edited-%d.ppm" "$task_edited"
cmp "$task_evidence/source-1.png" "$task_evidence/edited-1.png"
cmp "$task_evidence/source-2.png" "$task_evidence/edited-2.png"
python3 - "$task_evidence/edited-3.ppm" <<'PY'
import sys
from pathlib import Path

with Path(sys.argv[1]).open("rb") as stream:
    if stream.readline() != b"P6\n":
        raise ValueError("expected P6 PPM")
    width, height = map(int, stream.readline().split())
    if stream.readline() != b"255\n" or (width, height) != (240, 160):
        raise ValueError("unexpected pixel format or page size")
    stream.seek((110 * width + 140) * 3, 1)
    pixel = tuple(stream.read(3))
    if not (100 <= pixel[0] <= 160 and 195 <= pixel[1] <= 230 and 140 <= pixel[2] <= 190):
        raise ValueError(f"transparent replacement pixel mismatch: {pixel}")
PY
echo 'External-producer edit: structure, text, image position, path and untouched pages passed.'

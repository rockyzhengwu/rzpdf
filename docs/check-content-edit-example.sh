#!/usr/bin/env bash
# Exact edit_content example checks, not a general conformance/visibility validator.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-content-edit-example.sh path/to/example.pdf' >&2
    exit 2
fi
task_pdf_path=$1
for task_tool in qpdf mutool jq python3; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    deref($objects.trailer.value["/Root"]).value as $root |
    deref($root["/Pages"]).value as $tree |
    [$tree["/Kids"][] | deref(.).value] as $pages |
    [$pages[] | .["/Resources"]["/XObject"] | to_entries[] | .value] as $images |
    [$pages[] | .["/Resources"]["/Font"] | to_entries[] | .value] as $fonts |
    ($tree["/Count"] == 2) and ($pages | length == 2) and
    ($pages | all(.["/Type"] == "/Page" and .["/MediaBox"] == [0,0,360,400])) and
    ($pages[0]["/Contents"] != $pages[1]["/Contents"]) and
    ($images | length == 2) and ($images | unique | length == 1) and
    (deref($images[0]).stream.dict["/Subtype"] == "/Image") and
    (deref($images[0]).stream.dict["/Width"] == 2) and
    (deref($images[0]).stream.dict["/Height"] == 1) and
    ($fonts | length == 2) and ($fonts | unique | length == 1) and
    (deref($fonts[0]).value["/BaseFont"] == "/Helvetica")
' >/dev/null
task_text=$(mutool draw -q -F txt "$task_pdf_path" | tr -d '\f' | sed '/^[[:space:]]*$/d')
[[ "$task_text" == $'Original content\nEdited content' ]] || { echo 'Text/page order mismatch' >&2; exit 1; }
mutool draw -q -F trace "$task_pdf_path" | python3 -c '
import sys
import xml.etree.ElementTree as ET
root = ET.fromstring(sys.stdin.read())
pages = root.findall("page")
assert len(pages) == 2, "Expected two pages"
for page, image_count, colors in zip(pages, [2,1], [["1 0 0","0 0 1"],["0 0 1"]]):
    assert len(page.findall("fill_text")) == 1, "Text event mismatch"
    assert len(page.findall("fill_image")) == image_count, "Image draw count mismatch"
    assert [path.get("color") for path in page.findall("fill_path")] == colors, "Path paint mismatch"
    assert not page.findall("stroke_path"), "Unexpected stroke"
    assert not page.findall("clip_path"), "Unexpected clip"
old = pages[0].findall("fill_image")[0]
new = pages[1].findall("fill_image")[0]
assert [float(x) for x in old.get("transform").split()] == [160,0,0,40,40,180], "Original transform mismatch"
assert [float(x) for x in new.get("transform").split()] == [0,160,-40,0,140,120], "Edited transform mismatch"
assert ET.tostring(pages[0].findall("fill_path")[-1]) == ET.tostring(pages[1].findall("fill_path")[-1]), "Neighbor blue path changed"
'
echo 'Content edit example structure, shared resources, text and draw-event checks passed.'

#!/usr/bin/env bash
# Independent text positions for the fixed edit_cjk_text three-page sample.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-v02-cjk-edit.sh EDITED.pdf' >&2
    exit 2
fi
task_pdf=$1
for task_tool in qpdf mutool python3; do
    command -v "$task_tool" >/dev/null || { echo "Missing tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf"
mutool draw -q -F stext "$task_pdf" | python3 -c '
import math
import sys
import xml.etree.ElementTree as ET

root = ET.fromstring(sys.stdin.read())
pages = root.findall("page")
if len(pages) != 3:
    raise ValueError("expected three pages")
lines = [[(line.get("text"), line.findall(".//char")) for line in page.findall(".//line")]
         for page in pages]
if len(lines[0]) != 2 or len(lines[2]) != 2:
    raise ValueError("expected two CJK text lines on original and edited pages")
if [line[0] for line in lines[0]] != ["rzpdf - 中文字体嵌入", "你好，世界！Hello PDF"]:
    raise ValueError("original CJK text mismatch")
if [line[0] for line in lines[2]] != ["rzpdf - 中文字体嵌入", "你好，你好！Hello PDF"]:
    raise ValueError("edited CJK text mismatch")
for (old_text, old_chars), (new_text, new_chars) in zip(lines[0], lines[2]):
    if len(old_chars) != len(new_chars):
        raise ValueError("edited CJK character count changed")
    for old, new in zip(old_chars, new_chars):
        for axis in ("x", "y"):
            if not math.isclose(float(old.get(axis)), float(new.get(axis)), abs_tol=0.002):
                raise ValueError("fixed-width CJK edit shifted a character")
        if old_text == new_text and old.get("c") != new.get("c"):
            raise ValueError("unchanged title character changed")
'
echo 'CJK edit independent text and character positions passed.'

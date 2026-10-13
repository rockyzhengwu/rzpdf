#!/usr/bin/env bash
# Independent checks for Tj, TJ, single-quote and double-quote source edits.
set -euo pipefail
if [[ $# -ne 2 ]]; then
    echo 'Usage: bash docs/check-v02-text-operators.sh EDITED.pdf NEW_EVIDENCE_DIR' >&2
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
mutool draw -q -F stext "$task_pdf" | python3 -c '
import math
import sys
import xml.etree.ElementTree as ET

root = ET.fromstring(sys.stdin.read())
pages = root.findall("page")
if len(pages) != 2:
    raise ValueError("expected two pages")
expected = [["Uno", "XJ", "Prime", "Second"], ["One", "TJ", "Quote", "Double"]]
lines = []
for page in pages:
    page_lines = []
    for line in page.findall(".//line"):
        chars = line.findall(".//char")
        if chars:
            page_lines.append(("".join(ch.get("c", "") for ch in chars), chars[0]))
    lines.append(page_lines)
for page_lines, words in zip(lines, expected):
    if [word for word, _ in page_lines] != words:
        raise ValueError("text operator output mismatch")
for edited, original in zip(lines[0], lines[1]):
    for axis in ("x", "y"):
        if not math.isclose(float(edited[1].get(axis)), float(original[1].get(axis)), abs_tol=0.002):
            raise ValueError("text line origin moved")
'
mutool draw -q -r 96 -o "$task_evidence/text-%d.png" "$task_pdf"
echo 'Tj, TJ and both quote operators: independent text and line positions passed.'

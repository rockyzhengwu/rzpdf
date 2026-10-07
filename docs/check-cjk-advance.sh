#!/usr/bin/env bash
# Fixed compare_cjk_advance fixture only, not general layout/conformance checking.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-cjk-advance.sh comparison.pdf' >&2
    exit 2
fi
task_pdf_path=$1
for task_tool in qpdf mutool jq xmllint; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    deref($objects.trailer.value["/Root"]).value as $root |
    deref($root["/Pages"]).value as $tree |
    [$tree["/Kids"][] | deref(.).value] as $pages |
    [$pages[] | .["/Resources"]["/Font"] | to_entries[] | .value] as $fonts |
    [$pages[] | .["/Resources"]["/XObject"] | to_entries[] | .value] as $images |
    ($tree["/Count"] == 4) and ($pages | length == 4) and
    ($fonts | length == 4) and ($fonts | unique | length == 1) and
    (deref($fonts[0]).value["/Subtype"] == "/Type0") and
    (deref(deref($fonts[0]).value["/ToUnicode"]).stream.dict != null) and
    ($images | length == 4) and ($images[0] == $images[2]) and ($images[0] == $images[3]) and
    (deref($images[0]).stream.dict["/SMask"] == null) and
    (deref($images[1]).stream.dict["/SMask"] != null)
' >/dev/null
task_xml=$(mutool draw -F stext "$task_pdf_path")
for task_letter in P D F; do
    task_base_x=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[1]//char[@c='$task_letter'])[last()]/@x)" -)
    task_base_y=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[1]//char[@c='$task_letter'])[last()]/@y)" -)
    task_adjusted_x=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[3]//char[@c='$task_letter'])[last()]/@x)" -)
    task_adjusted_y=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[3]//char[@c='$task_letter'])[last()]/@y)" -)
    task_control_x=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[4]//char[@c='$task_letter'])[last()]/@x)" -)
    task_control_y=$(printf '%s' "$task_xml" | xmllint --xpath "string((//page[4]//char[@c='$task_letter'])[last()]/@y)" -)
    jq -en --argjson bx "$task_base_x" --argjson by "$task_base_y" \
        --argjson ax "$task_adjusted_x" --argjson ay "$task_adjusted_y" \
        --argjson ux "$task_control_x" --argjson uy "$task_control_y" '
        ($bx-$ax > -0.002 and $bx-$ax < 0.002) and
        ($by-$ay > -0.002 and $by-$ay < 0.002) and
        ($bx-$ux > 10) and ($by-$uy > -0.002 and $by-$uy < 0.002)
    ' >/dev/null
done
task_text=$(mutool draw -F txt "$task_pdf_path" | tr -d '[:space:]')
task_expected='rzpdf-中文字体嵌入你好，世界！HelloPDFPDF第二页-同一字体跨页复用中文可显示，也应正确提取。rzpdf-中文字体嵌入你好！PDFrzpdf-中文字体嵌入你好！PDF'
[[ "$task_text" == "$task_expected" ]] || { echo 'CJK comparison text/page order mismatch' >&2; exit 1; }
echo 'CJK comparison structure, independent trailing glyph positions and normalized text checks passed.'

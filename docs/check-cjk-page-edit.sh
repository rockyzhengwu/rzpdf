#!/usr/bin/env bash
# Fixed create_cjk --images -> edit_page_order/edit_cjk_text samples only.
set -euo pipefail
if [[ $# -lt 1 || $# -gt 2 || (${2:-} != '' && ${2:-} != '--text-edit') ]]; then
    echo 'Usage: bash docs/check-cjk-page-edit.sh edited.pdf [--text-edit]' >&2
    exit 2
fi
task_pdf_path=$1
task_text_edit=false
if [[ ${2:-} == '--text-edit' ]]; then task_text_edit=true; fi
for task_tool in qpdf mutool jq; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e --argjson text_edit "$task_text_edit" '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    deref($objects.trailer.value["/Root"]).value as $root |
    deref($root["/Pages"]).value as $tree |
    [$tree["/Kids"][] | deref(.).value] as $pages |
    [$pages[] | .["/Resources"]["/Font"] | to_entries[] | .value] as $fonts |
    [$pages[] | .["/Resources"]["/XObject"] | to_entries[] | .value] as $images |
    deref($fonts[0]).value as $font |
    deref($font["/DescendantFonts"][0]).value as $cid |
    deref($cid["/FontDescriptor"]).value as $descriptor |
    ($tree["/Count"] == 3) and ($pages | length == 3) and
    ([$pages[] | select(.["/Type"] == "/Page")] | length == 3) and
    ($fonts | length == 3) and ($fonts | unique | length == 1) and
    ($font["/Subtype"] == "/Type0") and ($font["/Encoding"] == "/Identity-H") and
    ($cid["/Subtype"] == "/CIDFontType2") and
    (deref($descriptor["/FontFile2"]).stream.dict["/Length1"] > 0) and
    (deref($font["/ToUnicode"]).stream.dict != null) and
    ($images | length == 3) and
    ([$images[] | deref(.).stream.dict | select(.["/Subtype"] == "/Image" and
        .["/Width"] == 2 and .["/Height"] == 2 and .["/BitsPerComponent"] == 8 and
        .["/ColorSpace"] == "/DeviceRGB")] | length == 3) and
    ($images[0] != $images[1]) and
    (if $text_edit then $images[0] == $images[2] else $images[1] == $images[2] end) and
    (deref($images[0]).stream.dict["/SMask"] == null) and
    (deref($images[1]).stream.dict["/SMask"] != null) and
    (deref(deref($images[1]).stream.dict["/SMask"]).stream.dict["/ColorSpace"] == "/DeviceGray")
' >/dev/null
task_text=$(mutool draw -F txt "$task_pdf_path" | tr -d '\f' | sed '/^[[:space:]]*$/d')
task_expected=$'rzpdf - 中文字体嵌入\n你好，世界！Hello PDF\n第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。\n第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。'
if [[ $task_text_edit == true ]]; then
    task_expected=$'rzpdf - 中文字体嵌入\n你好，世界！Hello PDF\n第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。\nrzpdf - 中文字体嵌入\n你好，你好！Hello PDF'
fi
[[ "$task_text" == "$task_expected" ]] || { echo 'Edited CJK text/page order mismatch' >&2; exit 1; }
echo 'Edited CJK page order, shared font/image and exact text checks passed.'

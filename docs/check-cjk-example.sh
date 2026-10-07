#!/usr/bin/env bash
# Read-only checks for the exact two-page create_cjk example, not general PDF conformance.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-cjk-example.sh path/to/example.pdf' >&2
    exit 2
fi
task_pdf_path=$1
for task_tool in qpdf mutool jq; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    $objects.trailer.value as $trailer |
    deref($trailer["/Root"]).value as $root |
    deref($root["/Pages"]).value as $tree |
    [$objects[] | .value? | select(.["/Type"] == "/Page")] as $pages |
    [$pages[] | .["/Resources"]["/Font"] | to_entries[] | .value] as $refs |
    deref($refs[0]).value as $font |
    deref($font["/DescendantFonts"][0]).value as $cid |
    deref($cid["/FontDescriptor"]).value as $descriptor |
    deref($descriptor["/FontFile2"]).stream.dict as $file |
    ($tree["/Count"] == 2) and ($pages | length == 2) and
    ($refs | length == 2) and ($refs | unique | length == 1) and
    ($font["/Subtype"] == "/Type0") and ($font["/Encoding"] == "/Identity-H") and
    ($font["/DescendantFonts"] | length == 1) and
    ($cid["/Subtype"] == "/CIDFontType2") and
    ($cid["/CIDSystemInfo"]["/Registry"] == "u:Adobe") and
    ($cid["/CIDSystemInfo"]["/Ordering"] == "u:Identity") and
    ($cid["/W"] | length > 0) and
    ($descriptor["/Type"] == "/FontDescriptor") and
    ($descriptor["/FontName"] == $font["/BaseFont"]) and
    ($file["/Length1"] > 0) and
    (deref($font["/ToUnicode"]).stream.dict != null) and
    (deref($cid["/CIDToGIDMap"]).stream.dict != null) and
    ([$objects[] | .value? | select(.["/FontFile2"] != null)] | length == 1) and
    (deref($trailer["/Info"]).value["/Title"] == "u:rzpdf 中文构建验证")
' >/dev/null
task_text=$(mutool draw -F txt "$task_pdf_path" | tr -d '\f' | sed '/^[[:space:]]*$/d')
task_expected=$'rzpdf - 中文字体嵌入\n你好，世界！Hello PDF\n第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。'
[[ "$task_text" == "$task_expected" ]] || { echo 'CJK example extracted text or page order mismatch' >&2; exit 1; }
echo 'CJK example structure, shared embedded font, Unicode title and exact text checks passed.'

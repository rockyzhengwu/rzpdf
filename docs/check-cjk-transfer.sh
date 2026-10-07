#!/usr/bin/env bash
# Fixed CJK --images fixtures only: reverse extraction or two-source merge.
set -euo pipefail
if [[ $# -ne 2 || ($1 != 'extract' && $1 != 'merge') ]]; then
    echo 'Usage: bash docs/check-cjk-transfer.sh extract|merge output.pdf' >&2
    exit 2
fi
task_mode=$1
task_pdf_path=$2
task_first=$'rzpdf - 中文字体嵌入\n你好，世界！Hello PDF'
task_second=$'第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。'
if [[ $task_mode == extract ]]; then
    task_masks='[true,false]'
    task_font_count=1
    task_expected="$task_second"$'\n'"$task_first"
else
    task_masks='[false,true,false,true,true]'
    task_font_count=2
    task_expected="$task_first"$'\n'"$task_second"$'\n'"$task_first"$'\n'"$task_second"$'\n'"$task_second"
fi
for task_tool in qpdf mutool jq; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e \
    --argjson masks "$task_masks" --argjson font_count "$task_font_count" '
    .qpdf[1] as $objects |
    def deref($ref): $objects["obj:" + $ref];
    deref($objects.trailer.value["/Root"]).value as $root |
    deref($root["/Pages"]).value as $tree |
    [$tree["/Kids"][] | deref(.).value] as $pages |
    [$pages[] | .["/Resources"]["/Font"] | to_entries[] | .value] as $fonts |
    [$pages[] | .["/Resources"]["/XObject"] | to_entries[] | .value |
        deref(.).stream.dict] as $images |
    ($tree["/Count"] == ($masks | length)) and
    ($pages | length == ($masks | length)) and
    ([$pages[] | select(.["/Type"] == "/Page")] | length == ($masks | length)) and
    ($fonts | length == ($masks | length)) and
    ($fonts | unique | length == $font_count) and
    ([$fonts[] | deref(.).value | select(.["/Subtype"] == "/Type0" and
        .["/Encoding"] == "/Identity-H" and
        deref(.["/ToUnicode"]).stream.dict != null)] | length == ($masks | length)) and
    ([$fonts[] | deref(.).value["/DescendantFonts"][0] | deref(.).value |
        select(.["/Subtype"] == "/CIDFontType2") | deref(.["/FontDescriptor"]).value |
        deref(.["/FontFile2"]).stream.dict | select(.["/Length1"] > 0)] |
        length == ($masks | length)) and
    ($images | length == ($masks | length)) and
    ([$images[] | select(.["/Subtype"] == "/Image" and .["/Width"] == 2 and
        .["/Height"] == 2 and .["/BitsPerComponent"] == 8 and
        .["/ColorSpace"] == "/DeviceRGB")] | length == ($masks | length)) and
    ([$images[] | .["/SMask"] != null] == $masks) and
    ([$images[] | select(.["/SMask"] != null) | deref(.["/SMask"]).stream.dict |
        select(.["/Subtype"] == "/Image" and .["/Width"] == 2 and .["/Height"] == 2 and
        .["/BitsPerComponent"] == 8 and .["/ColorSpace"] == "/DeviceGray")] |
        length == ([$masks[] | select(.)] | length))
' >/dev/null
task_text=$(mutool draw -F txt "$task_pdf_path" | tr -d '\f' | sed '/^[[:space:]]*$/d')
[[ "$task_text" == "$task_expected" ]] || { echo 'Transferred CJK text/page order mismatch' >&2; exit 1; }
echo "CJK $task_mode font/image/mask structure and exact page text checks passed."

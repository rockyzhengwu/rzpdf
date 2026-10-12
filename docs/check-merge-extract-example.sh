#!/usr/bin/env bash
# Exact checks for documented sample inputs; not a general merge verifier.
set -euo pipefail
if [[ $# -lt 2 || $# -gt 3 || (${3:-} != '' && ${3:-} != '--cjk-images') ]]; then
    echo 'Usage: bash docs/check-merge-extract-example.sh MERGED.pdf EXTRACTED.pdf [--cjk-images]' >&2
    exit 2
fi
for task_tool in qpdf mutool jq; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
task_merged=$1
task_extracted=$2
task_merged_images=2
task_extracted_images=1
if [[ ${3:-} == '--cjk-images' ]]; then
    # Two CJK images plus their SMask, and two English images.
    task_merged_images=5
    # Selected pages 3/0 contain one English and one opaque CJK image.
    task_extracted_images=2
fi
qpdf --check "$task_merged"
qpdf --check "$task_extracted"
qpdf --json --json-key=qpdf "$task_merged" | jq -e --argjson image_count "$task_merged_images" '
    .qpdf[1] as $o |
    ([$o[] | .value? | select(.["/Type"] == "/Page")] | length == 4) and
    ([$o[] | .value? | select(.["/FontFile2"] != null)] | length == 1) and
    ([$o[] | .stream.dict? | select(.["/Subtype"] == "/Image")] | length == $image_count)
' >/dev/null
qpdf --json --json-key=qpdf "$task_extracted" | jq -e --argjson image_count "$task_extracted_images" '
    .qpdf[1] as $o |
    ([$o[] | .value? | select(.["/Type"] == "/Page")] | length == 2) and
    ([$o[] | .value? | select(.["/FontFile2"] != null)] | length == 1) and
    ([$o[] | .stream.dict? | select(.["/Subtype"] == "/Image")] | length == $image_count)
' >/dev/null
task_merged_text=$(mutool draw -F txt "$task_merged" | tr -d '\f' | sed '/^[[:space:]]*$/d')
task_extracted_text=$(mutool draw -F txt "$task_extracted" | tr -d '\f' | sed '/^[[:space:]]*$/d')
task_expected_merged=$'rzpdf - 中文字体嵌入\n你好，世界！Hello PDF\n第二页 - 同一字体跨页复用\n中文可显示，也应正确提取。\nrzpdf - Creation example\nPage 1 | Text, image and metadata\nrzpdf - Creation example\nPage 2 | Text, image and metadata'
task_expected_extracted=$'rzpdf - Creation example\nPage 2 | Text, image and metadata\nrzpdf - 中文字体嵌入\n你好，世界！Hello PDF'
[[ "$task_merged_text" == "$task_expected_merged" ]] || { echo 'Merged text or page order mismatch' >&2; exit 1; }
[[ "$task_extracted_text" == "$task_expected_extracted" ]] || { echo 'Extracted text or 3,0 page order mismatch' >&2; exit 1; }
echo 'Merge/extract example page counts, font/image counts and exact text checks passed.'

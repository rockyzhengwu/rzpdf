#!/usr/bin/env bash
# Read-only checks for create_user_unit's exact fixture, not arbitrary PDFs.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-user-unit-example.sh SAMPLE.pdf' >&2
    exit 2
fi
for task_tool in qpdf mutool jq; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
task_sample=$1
qpdf --check "$task_sample"
qpdf --json "$task_sample" | jq -e '
    .qpdf[1] as $objects |
    (.qpdf[0].pdfversion == "1.6") and
    (.pages | length == 6) and
    ([.pages[] | $objects["obj:" + .object].value |
        [.["/UserUnit"], .["/Rotate"], .["/MediaBox"], .["/CropBox"]]] ==
        [[1.5,0,[0,0,200,140],[10,20,170,100]],
         [1.5,90,[0,0,200,140],[10,20,170,100]],
         [1.5,180,[0,0,200,140],[10,20,170,100]],
         [1.5,270,[0,0,200,140],[10,20,170,100]],
         [0.5,0,[0,0,200,140],[10,20,170,100]],
         [2.5,0,[0,0,200,140],[10,20,170,100]]]) and
    (all(.pages[]; (.images | length == 1) and
        (.images[0] | .width == 2 and .height == 1 and
         .bitspercomponent == 8 and .colorspace == "/DeviceRGB")))
' >/dev/null
task_text=$(mutool draw -F txt "$task_sample" | tr -d '\f' | sed '/^[[:space:]]*$/d')
task_expected=$'UserUnit 1.5, Rotate 0\nUserUnit 1.5, Rotate 90\nUserUnit 1.5, Rotate 180\nUserUnit 1.5, Rotate 270\nUserUnit 0.5, Rotate 0\nUserUnit 2.5, Rotate 0'
[[ "$task_text" == "$task_expected" ]] || { echo 'UserUnit text or page order mismatch' >&2; exit 1; }
echo 'UserUnit fixture version, ordered page geometry, image metadata and exact text checks passed.'

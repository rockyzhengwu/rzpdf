#!/usr/bin/env bash
# Reproducible v0.2 content-editing gate; requires the v0.1 font inputs.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-v02.sh NEW_EVIDENCE_DIRECTORY' >&2
    exit 2
fi
task_repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_output=$1
mkdir "$task_output"
task_output=$(cd "$task_output" && pwd)
cd "$task_repo"
exec > >(tee "$task_output/gate.log") 2>&1
trap 'task_exit=$?; printf "gate_exit=%s\n" "$task_exit" > "$task_output/result.txt"' EXIT
printf 'Started UTC: '; date -u
git rev-parse HEAD
git status --short

# The v0.1 gate runs the locked full workspace suite and creation/read/write
# checks, and records source hashes plus its own exit status.
bash docs/check-v01.sh "$task_output/v01"
[[ $(< "$task_output/v01/result.txt") == 'gate_exit=0' ]]

cargo run -p pdf --example edit_content --locked -- "$task_output/content-edited.pdf"
bash docs/check-content-edit-example.sh "$task_output/content-edited.pdf"
mutool draw -q -r 96 -o "$task_output/content-%d.png" "$task_output/content-edited.pdf"

cargo run -p pdf --example edit_text_operators_v02 --locked -- "$task_output/text-operators.pdf"
bash docs/check-v02-text-operators.sh \
    "$task_output/text-operators.pdf" "$task_output/text-operators-check"

cargo run -p pdf --example edit_cjk_text --locked -- \
    "$task_output/v01/cjk.pdf" "$task_output/cjk-edited.pdf"
bash docs/check-cjk-page-edit.sh "$task_output/cjk-edited.pdf" --text-edit
bash docs/check-v02-cjk-edit.sh "$task_output/cjk-edited.pdf"
mutool draw -q -r 96 -o "$task_output/cjk-source-%d.png" "$task_output/v01/cjk.pdf"
mutool draw -q -r 96 -o "$task_output/cjk-edit-%d.png" "$task_output/cjk-edited.pdf"
cmp "$task_output/cjk-source-1.png" "$task_output/cjk-edit-1.png"
cmp "$task_output/cjk-source-2.png" "$task_output/cjk-edit-2.png"

cargo run -p pdf --example edit_external_v02 --locked -- \
    pdf/tests/fixtures/v01/mupdf.pdf "$task_output/external-edited.pdf"
bash docs/check-v02-external-example.sh \
    pdf/tests/fixtures/v01/mupdf.pdf "$task_output/external-edited.pdf" \
    "$task_output/external-check"

cargo run -p pdf --example edit_form_v02 --locked -- "$task_output/form-edited.pdf"
bash docs/check-v02-form-example.sh \
    "$task_output/form-edited.pdf" "$task_output/form-check"

shasum -a 256 -c "$task_output/v01/source.sha256"
echo 'Automated v0.2 gate passed; review new page renders and the frozen support matrix before acceptance.'

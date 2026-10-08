#!/usr/bin/env bash
# Fixed four-page compare_cjk_advance --events fixtures, not a PDF validator.
set -euo pipefail
if [[ $# -ne 2 ]]; then
    echo 'Usage: bash docs/check-cjk-event-negatives.sh TOP_LEVEL.pdf NESTED.pdf' >&2
    exit 2
fi
for task_tool in qpdf rg; do
    command -v "$task_tool" >/dev/null || { echo "Missing external tool: $task_tool" >&2; exit 2; }
done
task_script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
task_checker="$task_script_dir/check-cjk-advance.sh"
# Positive controls prevent missing dependencies or bad inputs masquerading as
# successful rejection. Neither input is modified.
bash "$task_checker" "$1"
bash "$task_checker" "$2" --nested
task_temp=$(mktemp -d "${TMPDIR:-/tmp}/rzpdf-cjk-event-negatives.XXXXXX")
trap 'rm -f -- "$task_temp/top-swapped.pdf" "$task_temp/nested-swapped.pdf" "$task_temp/top.log" "$task_temp/nested.log"; rmdir -- "$task_temp"' EXIT
qpdf --empty --pages "$1" 1,2,4,3 -- "$task_temp/top-swapped.pdf"
qpdf --empty --pages "$2" 1,2,4,3 -- "$task_temp/nested-swapped.pdf"
for task_mode in top nested; do
    task_pdf="$task_temp/$task_mode-swapped.pdf"
    qpdf --check "$task_pdf"
    task_flags=()
    if [[ "$task_mode" == nested ]]; then task_flags=(--nested); fi
    task_status=0
    bash "$task_checker" "$task_pdf" "${task_flags[@]}" > "$task_temp/$task_mode.log" 2>&1 || task_status=$?
    if [[ $task_status -ne 1 ]]; then
        echo "$task_mode: expected comparison rejection (exit 1), got $task_status" >&2
        exit 1
    fi
    # Require the checker to reach independent four-page coordinate extraction,
    # not fail earlier on syntax, resource structure or missing tools.
    rg -q '^page .* 4$' "$task_temp/$task_mode.log"
    if rg -q 'CJK comparison structure, independent trailing glyph positions and normalized text checks passed' "$task_temp/$task_mode.log"; then
        echo "$task_mode: swapped controls were incorrectly accepted" >&2
        exit 1
    fi
    echo "$task_mode: valid swapped comparison rejected at coordinate stage"
done

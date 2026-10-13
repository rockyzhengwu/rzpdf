#!/usr/bin/env bash
# Reproducible, non-fuzz release gate. Run from any working directory.
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-v01.sh NEW_EVIDENCE_DIRECTORY' >&2
    exit 2
fi
task_repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_output=$1
# Never overwrite previous evidence or existing user files.
mkdir "$task_output"
task_output=$(cd "$task_output" && pwd)
cd "$task_repo"
exec > >(tee "$task_output/gate.log") 2>&1
trap 'task_exit=$?; printf "gate_exit=%s\n" "$task_exit" > "$task_output/result.txt"' EXIT
printf 'Started UTC: '; date -u
git rev-parse HEAD
git status --short
git diff --binary > "$task_output/tracked.patch"
git ls-files --others --exclude-standard > "$task_output/untracked.txt"
# Include untracked implementation, fixtures and gate scripts, unlike HEAD alone.
task_sources() {
    rg --files --hidden -g '*.rs' -g Cargo.toml -g Cargo.lock -g 'pdf/tests/fixtures/v01/**' \
        -g 'docs/check-*.sh' -g 'docs/check-*.py' -g 'docs/generate-v01-fixture.sh' \
        -g '.github/workflows/ci.yml' -g 'font-data/cmap-provenance.json' \
        -g 'font-data/cmaps/**' -g 'font-data/fonts/**' -g 'font-data/afm/**' \
        -g 'font-data/licenses/**' -g 'docs/licenses/**' \
        -g '!.git/**' -g '!fuzz/**' -g '!target/**' -g '!output/**' -g '!tmp/**' \
        -g '!font-data/src/cmap.rs' -g '!font-data/src/fonts/**' -g '!font-data/src/afm/**' |
        LC_ALL=C sort
}
task_sources > "$task_output/source-files.txt"
while IFS= read -r task_file; do
    shasum -a 256 "$task_file"
done < "$task_output/source-files.txt" > "$task_output/source.sha256"
rustc --version
cargo --version
for task_tool in qpdf mutool jq xmllint python3; do
    command -v "$task_tool" >/dev/null || { echo "Missing tool: $task_tool" >&2; exit 2; }
done
qpdf --version
mutool -v
python3 docs/check-cmap-data.py
qpdf --check pdf/tests/fixtures/v01/mupdf.pdf
# Validate required inputs before starting the expensive full regression.
: "${RZPDF_CJK_FONT:?Supply a licensed static TrueType CJK font}"
: "${RZPDF_CJK_NAME:?Supply its PostScript name}"
: "${RZPDF_CJK_CAP_HEIGHT:?Supply cap height in 1000 units}"
: "${RZPDF_CJK_STEM_V:?Supply stem width in 1000 units}"
[[ -f "$RZPDF_CJK_FONT" ]] || { echo 'CJK font is not a regular file' >&2; exit 2; }
shasum -a 256 "$RZPDF_CJK_FONT"
cargo test --workspace --locked
cargo run -p pdf --example create_document --locked -- "$task_output/created.pdf"
bash docs/check-creation-example.sh "$task_output/created.pdf"
mutool draw -r 72 -o "$task_output/created-%d.png" "$task_output/created.pdf"
# CJK gate is mandatory: no silent skip when no licensed font was supplied.
cargo run -p pdf --example create_cjk --locked -- "$RZPDF_CJK_FONT" "$RZPDF_CJK_NAME" "$RZPDF_CJK_CAP_HEIGHT" "$RZPDF_CJK_STEM_V" "$task_output/cjk.pdf" --images
bash docs/check-cjk-example.sh "$task_output/cjk.pdf" --images
cargo run -p pdf --example merge_documents --locked -- "$task_output/merged.pdf" "$task_output/cjk.pdf" "$task_output/created.pdf"
cargo run -p pdf --example extract_pages --locked -- "$task_output/selected.pdf" "$task_output/merged.pdf" 3 0
bash docs/check-merge-extract-example.sh "$task_output/merged.pdf" "$task_output/selected.pdf" --cjk-images
for task_name in cjk merged selected; do
    mutool draw -r 72 -o "$task_output/$task_name-%d.png" "$task_output/$task_name.pdf"
done
mutool draw -r 72 -o "$task_output/independent-%d.png" pdf/tests/fixtures/v01/mupdf.pdf
task_sources > "$task_output/source-files-final.txt"
cmp "$task_output/source-files.txt" "$task_output/source-files-final.txt"
shasum -a 256 -c "$task_output/source.sha256"
echo 'Automated gate passed. Visual review, font/sample license registration and candidate commit verification remain mandatory.'

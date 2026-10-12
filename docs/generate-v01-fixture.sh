#!/usr/bin/env bash
# Rebuild the project-authored compatibility sample with an independent producer.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
task_fixture=pdf/tests/fixtures/v01
command -v mutool >/dev/null
command -v qpdf >/dev/null
mutool -v
mutool draw -r 72 -o "$task_fixture/pixels.png" "$task_fixture/pixels.svg"
mutool create -O compress,regenerate-id=no -o "$task_fixture/mupdf.pdf" "$task_fixture/page1.txt" "$task_fixture/page2.txt"
qpdf --check "$task_fixture/mupdf.pdf"
shasum -a 256 "$task_fixture/mupdf.pdf" "$task_fixture/pixels.png"

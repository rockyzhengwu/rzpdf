#!/usr/bin/env bash
# Read-only verifier for the fixed two-page overlay example, not a general validator.
set -euo pipefail
if [[ $# -ne 1 ]]; then
  echo 'Usage: bash docs/check-overlay-example.sh OVERLAY.pdf' >&2
  exit 2
fi
command -v qpdf >/dev/null
command -v mutool >/dev/null
command -v python3 >/dev/null
qpdf --check "$1"
mutool draw -F txt "$1" | python3 -c '
import sys
text = sys.stdin.read()
pages = text.split("\f")
if pages and not pages[-1].strip():
    pages.pop()
expected = [
    ["rzpdf - Creation example", "Page 1 | Text, image and metadata", "DRAFT", "Page 1 / 2"],
    ["rzpdf - Creation example", "Page 2 | Text, image and metadata", "DRAFT", "Page 2 / 2"],
]
actual = [[line.strip() for line in page.splitlines() if line.strip()] for page in pages]
if actual != expected:
    print("Overlay page text/order mismatch", file=sys.stderr)
    sys.exit(1)
print("Fixed overlay sample: two pages, original text, watermarks and page numbers verified")
'

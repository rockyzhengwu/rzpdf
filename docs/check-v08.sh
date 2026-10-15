#!/bin/sh
# Local acceptance entry: no downloads, installs, Poppler, fuzz, git mutation.
set -eu
cd "$(dirname "$0")/.."
v08_python="${V08_PYTHON:-python3.11}"
command -v qpdf >/dev/null
command -v mutool >/dev/null
"$v08_python" -c 'import PIL, pikepdf; from PIL import ImageCms'
printf '%s\n' 'v0.8: full workspace regression (includes slow subset/curve tests)'
cargo test --workspace --locked -j2 -- --test-threads=2
printf '%s\n' 'v0.8: building independent sample producers and renderer'
cargo build -p pdf --example v08_patterns --example v08_tensor \
  --example v08_groups --example v08_masks --example v08_cms \
  --example v08_text_clip --example v08_combinations --example create_shading \
  -p render --example render_document --locked -j2
for v08_gate in patterns tensor groups masks cms text-clip combinations shadings; do
  printf 'v0.8: independent gate %s\n' "$v08_gate"
  "$v08_python" "docs/check-v08-${v08_gate}.py"
done
git diff --check
printf '%s\n' 'v0.8: automated gates passed; inspect the generated PNGs and record evidence'

# Bounded CoreText font-manager initialization

Date: 2026-10-27. Baseline: `fd6ded0`, following the
[coverage blending round](coverage-blending-performance.md).

The memory-font loader now supplies Skia's public CoreText factory with a
collection containing one resolved system UI-font descriptor. This avoids
the factory's explicit enumeration of every installed font family. It still
performs an initial CoreText UI-font query; initialization is not free and
remains inside first-render timing.

The UI font is a collection anchor, never a PDF substitute. Both embedded and
explicitly registered fonts still load the supplied program and collection
index through the same CoreText `new_from_data` implementation. Invalid input
continues to fail. An empty collection is deliberately avoided: CoreText
returns a null matching-descriptor array for it, which this Skia factory
assumes is non-null. The bridge validates a single matching descriptor and
uses the existing factory if construction fails.

The small C++ bridge uses the version-matched public header and the Rust
wrapper's supported opaque-pointer ownership API. Eleven unmodified headers
(about 51 KiB) and their BSD license/hash manifest are bundled under
[render/native/skia-headers](../render/native/skia-headers/README.md).
Both Skia Rust dependencies are pinned to 0.93.1. Custom native Skia debug,
source, library-path or GN configurations retain the old factory rather than
assuming their `sk_sp` return ABI matches the default release binaries.
Other targets keep the custom-empty-manager-or-default policy.

The session manager remains lazy, charged to the same cache allowance, and
subject to clearing and document/revision invalidation. Font face budgets,
the 128-face retention ceiling, font selection and missing-glyph policy have
not been expanded. No system fonts are registered globally.

## Independent native comparison

Seven existing text cases at 72/144 DPI, five interleaved trials per binary,
produced 140 successful runs. There were no warmups or repeated warm renders
in this font-specific comparison. All 30 selected page outputs were
byte-identical. First-output after/before geometric mean was **0.618**,
or approximately **38.2% less time** on these 14 text combinations.

Selected 144 DPI medians, milliseconds:

| Case | Before | After |
|---|---:|---:|
| Dense Latin text | 62.49 | 43.72 |
| Embedded TrueType | 48.64 | 32.86 |
| Dense CJK | 49.29 | 32.26 |

Separate attribution probes measured first manager construction at 16.27 ms
for the bounded collection versus 26.86 ms for the system manager, then
approximately 0.04–0.06 ms versus 16.7–17.8 ms for subsequent constructions.
Family counts were one versus 178; embedded font loading and glyph IDs were
preserved. These two probe processes attribute costs; their times are not an
engine ranking. User applications remained active and absolute times vary.

The font-only implementation passed 107 native render library tests and
strict workspace/all-target Clippy. The final combined native/browser
verification, curve measurements and real-file results are recorded in the
[first-render follow-up](first-render-performance.md).

Artifacts: ignored `output/coretext-collection-20261027/font-pair/`,
`profile-memory.json`, `profile-system.json` and native test/build logs.
Reproduce attribution after building the release example:

```sh
target/release/examples/profile_font_backend \
  pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf 5 memory
target/release/examples/profile_font_backend \
  pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf 5 system
```

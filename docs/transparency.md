# Transparency and text clipping

The core interpreter emits explicit `GroupBegin(GroupPaint)` / `GroupEnd` events.
Leaf objects retain their source provenance and text remains extractable. A
custom `Device` must implement `begin_group` and `end_group`; the default returns
`UnsupportedFeature`, never silently flattens a group. `end_group(false)` discards
its members and consumes the boundary even when cleanup fails. Display-list
replay unwinds only the groups it opened. `replay_contents` replays inside an
already started page; the caller owns surrounding group/page boundaries.

## RGB compositing

Skia supports isolated and non-isolated Form transparency groups, including
knockout and nesting. Isolated Form groups accept `DeviceRGB`, the restricted
DeviceGray subset below, and ICCBased RGB matrix/TRC profiles with component
ranges `[0, 1]`. Indirect `CS` arrays and profile streams use the ordinary colour
space parser. Paint colours, images, shadings and pattern renderers target the
current group profile; the composed, unpremultiplied group colours are converted
to the parent's space before applying group opacity and the parent blend mode.
Alpha is preserved. ICC groups are not aliases for DeviceRGB: linear-light
black/white mixing at 50% produces about 188 in sRGB, rather than 128.

Absent `CS` inherits the parent's working space. Non-isolated Form groups inherit
their backdrop's space; their `CS` entry does not select a new one. Nested explicit
DeviceRGB groups select nominal sRGB and restore the enclosing ICC context on
success or discard. ICC Gray/CMYK groups, LUT-only output profiles, calibrated
group spaces, overprint simulation and spot separation remain unsupported. This
does not add a print-proof pipeline or change the 8-bit raster precision.

Groups use separate offscreen surfaces. A non-isolated group's initial backdrop
is retained separately from its running result; if its parent is knockout it
inherits the parent's initial backdrop. Independent accumulated opacity removes
that backdrop before applying group alpha, blend mode, clip and soft mask once.
Knockout additionally tracks shape independently of opacity; a zero-opacity
member can still knock out earlier members. The default text knockout behaviour
treats individual glyphs as members. Nondefault `TK` and `AIS` are not supported.
Combined fill/stroke operations with transparency use an implicit non-isolated
knockout group, rather than blending the stroke onto a translucent fill.

All 16 standard blend modes use Skia's RGB implementations for paths, images
and drawable embedded text. Coverage/antialiasing and 8-bit unpremultiplication
can differ across readers; the independent gate records those differences and
checks numerical compositing separately. It does not claim identical pixels.

## Graphics soft masks

`ExtGState /SMask` supports `/None` or a mask dictionary with `/S /Alpha` or
`/Luminosity`, a transparency Form `G`, optional RGB `BC`, and `TR` (`Identity`
or a supported one-input/one-output PDF function). Luminosity requires an
explicit `DeviceRGB` group space. The current supported luminosity evaluation
uses nominal RGB coefficients 0.30, 0.59 and 0.11; it is not ICC RGB-to-gray.
The isolated group's colour is evaluated on transparent, then composited over
`BC` for luminosity reduction, so `BC` does not become a member's backdrop.
Alpha ignores backdrop colour. Knockout in the mask group is preserved.

The immutable interpreted mask is anchored to the CTM at `gs`; later `cm`
does not move it. Replacement, `/None`, and `q/Q` are ordinary graphics state
operations. Paths, text, images, shading and groups multiply their opacity by
the mask. Mask text is not reported as page text or page-edit provenance.
Unsupported operations, malformed dictionaries and cycles return errors.
ICC soft-mask group working spaces remain explicit errors; ordinary ICC Form
group support does not imply ICC luminosity-mask support.

## Character queries and colour conversion

Paint-order character extraction (including its explicit optional-content
variant) interprets Form geometry and text without evaluating group blending
spaces or compiling soft-mask graphics. Unsupported compositing therefore need
not block character positions, source addresses or Unicode. The scope restores
on errors and its temporary lists never enter rendering caches. Fonts, content
syntax, invocation limits and other graphics parsing still apply: this is not
a universal recovery or an image-free interpreter. Marked/semantic extraction
retains its existing full-validation contract. Browser `content()` also queries
images, so that combined API still requires a supported graphics display list.

## Text clipping

Interpreted modes 4–7 use actual unhinted simple/composite TrueType outlines.
Outlines from all text-show operations in one text object form one nonzero-winding
compound path, applied at `ET`, not intersected after each `Tj`. Modes 4/5/6
also paint fill/stroke/both before that new clip takes effect; mode 7 paints
nothing. Signed/zero sizes, `Tz`, affine transforms and `q/Q` are preserved.
An empty outline can produce an empty clip, not a bounding-box substitute.

CFF/Type1 and Type3 clipping are not supported by this outline path and fail
explicitly. Ordinary supported font painting is unchanged. A raw manually
constructed `TextObject` with a clipping mode cannot establish ET geometry:
use interpreter-produced lists (`has_interpreted_clipping()`), not a fabricated
flag or glyph bounds. Clipping state is retained on subsequent draw objects.

## Limits and errors

Recursion, content work, decoded mask data and retained mask/clip/Pattern payloads
are bounded. Offscreen surfaces, sibling shaders, correction Vecs/native copies
and nested groups conservatively subtract their live pixel/byte allowances
before allocation. A rejected group is discarded; a failed new page preserves
the preceding page. Native non-isolated/knockout pixel correction currently
rejects big-endian backends. OperationBudget does **not** cover all Skia/CMS/font
internals or every PDF function evaluation temporary; these are not RSS or
global CPU guarantees. Dense masks, groups and page-sized patterns are not a
throughput promise.

Identical decoded ICC profile bytes are interned per document revision in a
bounded LRU. Profiles share their transform tables across repeated colour-space
lookups; `clear_render_caches()` releases cache-owned ICC references as well as
page/font references. Caller-held lists can retain profiles. The browser build
also reserves estimated transform storage before calling the CMS backend:
32 MiB per Wasm module, shared by its documents. Exhaustion returns
`resource_limit`, reason `icc_transform_bytes`, stage `color_conversion`, with
numeric `limit`/`actual`; it does not intentionally abort the engine. This
conservative estimate is separate from raster budgets, not a complete CMS/RSS
allocator limit. Native transforms retain their existing policy.

See [v0.8 support matrix](releases/v0.8-support.md) for exact verification scope.
The compositing model follows [PDF 32000-1, sections 11.4–11.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).

## Page backdrop and text strokes

Page content blends against an initially transparent backdrop; the configured white output substrate is added after page compositing. Pattern/offscreen renderers retain their transparent output. Raw RGBA readback preserves straight alpha. Text outlines are transformed into graphics user space before stroking, so text-matrix scaling does not scale the graphics-state line width or dash pattern.

Embedded standalone CID-keyed CFF glyph selection follows the default Skia font
manager: CoreText on Apple and DirectWrite on Windows use physical GIDs;
FreeType ports use CIDs for standalone CID-keyed CFF and ordinary GIDs for
OpenType-wrapped CFF. The renderer therefore does not infer the drawing index
from raw program bytes alone. CID-to-GID charset lookup and PDF advances remain
separate. The compatibility regression uses sparse CID 42 / GID 2 and tests
visible ink; platform acceptance covers macOS CoreText and the pinned browser
FreeType build, not independent Windows/Linux certification.

Subpixel positive stroke widths use a one-device-pixel hairline when every principal CTM scale makes the requested width smaller than a pixel. This display policy keeps very thin rules visible; anisotropic strokes that are wide in any principal direction retain their user-space width. Zero/singular transforms are not promoted into painted hairlines by this rule.


## Corpus compatibility extensions

Isolated DeviceGray groups and DeviceGray luminosity masks accept DeviceGray
path/text paints, grayscale images and gray shadings. Equal RGB channels keep
this subset's blending arithmetic identical in the existing compositor. Colored
paints and patterns in these groups remain explicit errors until working-space
conversion is implemented. Gray mask BC has one component, replicated for
luminance evaluation; RGB BC retains three. Nested paints use the same check,
and failed groups are discarded rather than partially committed.

For a Type6/7 patch whose exact inverse point query reports unresolved geometry,
display can use forward device-space tessellation. Rounded control-hull flatness
and diagonal warp must total at most 1/8 device pixel per accepted cell. Geometry
and colors remain separate: source components/function parameters interpolate in
parameter space before function evaluation and color conversion. Greatest v then
u wins within a patch; a proven common-v identity selects greatest u without
epsilon tie merging; later patches paint over earlier ones.

The fallback is bounded by depth 12, a fixed maximum DFS stack, a per-pixel root
buffer, and explicit raster work including offscreen scans. Scratch is counted
alongside retained patch storage and the renderer's simultaneous raster/surface
allocation. A failed raster is discarded before painting. Exact color_at_point
queries retain their certified semantics and unresolved errors; this rendering
approximation is not an exact inverse-geometry certificate.

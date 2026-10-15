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
knockout and nesting. The explicit group blending space is currently limited to
`DeviceRGB`; absent `CS` inherits the supported RGB display model. Non-isolated
Form groups cannot declare their own `CS`. This is not CMYK, calibrated or ICC
group-space compositing, overprint simulation, spot separation or a print proof.
Unsupported explicit group spaces are rejected.

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

See [v0.8 support matrix](releases/v0.8-support.md) for exact verification scope.
The compositing model follows [PDF 32000-1, sections 11.4–11.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).

# Explicit colour conversion

`ColorSpace::rgb_with_context`, `Shading::color_at[_point]_with_context` and
`PdfImage::rgba_buffer_with_context` accept a `ColorConversionContext`. The
existing methods use a default sRGB output and RelativeColorimetric intent.
`with_output_profile` accepts an RGB `ICCProfile`; the context exposes an explicit
intent and `IccFallback` policy. These are numeric output APIs. Skia's final
display output uses canonical sRGB/nominal DeviceRGB; it does not write a
caller-selected output ICC into PNG. Isolated Form groups can use an actual
RGB matrix/TRC ICC working space and convert their composed colours back to the
parent space; this does not implement arbitrary group spaces. See
[transparency](transparency.md).

```rust,ignore
use pdf::color::{conversion::ColorConversionContext, icc_profile::ICCProfile};
let output = ICCProfile::try_new(&profile_bytes)?;
let context = ColorConversionContext::with_output_profile(output)?;
let rgb = color_space.rgb_with_context(&components, &context)?;
```

## Real ICC, not Alternate

The pinned pure-Rust moxcms backend reads actual profile bytes and transforms
Gray/RGB/CMYK ICCBased inputs. Supported profile envelopes are v2/v4 ordinary
input/display/output/colour-space classes with XYZ/Lab PCS supported by the
backend. Matrix/TRC and supported multidimensional LUT transforms can be used.
The positive regression corpus contains generated sRGB/AdobeRGB, sRGB-curve
gray and a project-authored synthetic CMYK LUT16 profile. This is a limited
tested set, not acceptance of every v2/v4 profile or a production print profile.
DeviceLink, named-colour and other component layouts are rejected. A backend
unsupported transform remains an explicit error by default.

`Perceptual`, `RelativeColorimetric`, `Saturation`, and `AbsoluteColorimetric`
propagate from `ri` / ExtGState `RI` and image `Intent`, with `q/Q` restoration.
Absolute intent additionally applies media-white scaling to an RGB matrix/TRC
output target; moxcms alone selects the colorimetric LUT without that scaling.
Absolute output to a non-matrix RGB target is explicitly unsupported. Numeric
checks use independent LittleCMS and preserve four-intent results.

Empty, truncated, bad-header/tag or component-mismatched ICCBased streams now
return errors; they are no longer silently interpreted as Alternate. Dictionary
Range is applied to component inputs and default colour. `IccFallback::Alternate`
must be selected explicitly for a valid but unsupported transform (or a legacy
programmatic ICCBased without a profile). It does not bypass parsing errors,
resource-limit errors, invalid input or poisoned caches, and is not advertised
as accurate ICC conversion.

Migration: `ICCProfile` is now an opaque shared parsed value; use `try_new`,
`srgb` or `gray_srgb` instead of an empty struct literal. The formerly inert
ICCBased default is not a valid strict profile. A test or application that used
empty profile bytes must provide a real configuration or deliberately choose
the documented programmatic fallback policy.

Indexed bases and Separation/DeviceN alternates propagate the same context.
Device colours retain the nominal display conversion, then transform to a
custom numeric RGB output if selected. Lab uses its existing white-point and
Bradford display conversion; nonzero BlackPoint compensation is still explicitly
unsupported. Separation/DeviceN use tint transforms and supported alternates,
not physical ink prediction, named-ink matching or plate separation. OutputIntent
selection/print-proof production is not added by this API.

## Bounded retention

Colour-space construction tracks active indirect references and rejects cycles
before recursively following the same space again. The recursive implementation
also caps colour nesting at 16, or the configured `max_depth` if smaller. This
implementation bound applies to direct definitions too, so a larger syntax limit
cannot exhaust the native default thread stack first. Both depth and reference
scopes restore on errors and native unwinding. It is not a promise to accept
arbitrarily nested alternate spaces.

One profile shares parsed data and an at-most-eight-entry output/intent transform
cache across clones/threads; profiles are not cached in an unbounded global map.
Document colour-space lookups intern identical decoded profile bytes in a
revision-scoped, bounded LRU. Cache clearing releases its owned references;
existing display lists can retain shared profiles. The Wasm build additionally
preflights transform-table storage against a module-wide 32 MiB conservative
estimate, returning a structured resource error on refusal; see
[browser resources](webassembly/resources.md#cache-observation-and-release).
Encoded profiles/tags are capped at the lesser of the configured decoded limit
and 16 MiB, CLUT input at 1 MiB, TRC elements at 40,000. Invalid tag extents are
rejected before backend parsing. Profile source bytes participate in logical
display/image/shading retained accounting, conservatively per draw. Backend
tables and transform temporaries are bounded by backend/input caps but are not
all leased to the shared OperationBudget. No total-process peak guarantee is made.

Dependencies, selected licenses and redistribution notices are in
[the dependency inventory](releases/v0.8-dependencies.md). No unknown-license
system ICC files are bundled. Synthetic LUTs follow [ICC lut16Type / PCS
encoding](https://www.color.org/ICC1V42.pdf); unlike lut8Type with XYZ PCS, this
has a defined numeric encoding. Rendering/white-point policy follows [ICC's
media-white explanation](https://www.color.org/whyd50/).

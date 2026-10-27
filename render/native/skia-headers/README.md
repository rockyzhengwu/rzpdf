# Skia headers for the CoreText bridge

These eleven unmodified headers and `LICENSE` are copied from the Skia source
shipped with `skia-bindings` **0.93.1**, Skia tag **m145-0.92.0**:

https://github.com/rust-skia/skia/tree/m145-0.92.0

`manifest.json` records SHA-256 for every copied file. Their original copyright
and BSD license notices remain intact. This directory is third-party code;
the repository's AGPL grant does not replace its license.

The small bridge compiles against the public `SkFontMgr_New_CoreText` API, not
handwritten class layouts or guessed C++ symbols. Both Rust Skia dependencies
are pinned to the matching version. Update and verify all three together when
upgrading Skia. Only macOS compiles the bridge. Custom Skia debug/source/GN or
system-library builds retain the existing manager factory because `sk_sp`'s
C++ return ABI can depend on the native build configuration.

The resolved system UI font supplies one descriptor to the collection; it is
never selected for PDF text. Empty collections cannot be passed to this Skia
version: CoreText returns a null matching-descriptor array which the factory
does not handle. The bridge validates a nonempty single-descriptor result and
otherwise returns null, allowing the Rust caller to use its previous factory.

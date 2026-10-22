# rzpdf

**Build, read, and work with PDFs in Rust.**

rzpdf is an evolving PDF toolkit that brings document creation, content extraction, page organization, editing, and rendering into one object model.

It is designed for Rust applications that need to generate PDFs, extract content, or work with pages and document resources.

## From creation to editing

- **Create documents:** Set page sizes, draw text, paths, and images, and write Info or XMP metadata.
- **Read and extract:** Parse documents, inspect page geometry, extract text, images, and drawing positions, and review parser diagnostics.
- **Organize pages:** Add, remove, copy, reorder, rotate, and crop pages; import pages across documents, merge files, or select pages in a specified order.
- **Open and save:** Load from a file or bytes, choose full rewrites or explicit append-only saves, and preserve encoded streams or compress unencoded streams within the documented input limits.
- **Protect and sign:** Read and write AES-256 R6 protection, verify historical CMS signatures, prepare visible or invisible approvals for an external signer, and apply bounded updates subject to signature constraints. Private keys stay outside the PDF core.
- **Render and edit:** Use a separate Skia renderer, or edit text, images, and paths within the documented font and source limits, including targeted edits in one-level, non-Group Forms.

Chinese text creation supports embedded static TrueType fonts. Standard Western fonts have explicit text-encoding APIs. [Positioned glyph runs](docs/glyph-runs.md) let applications supply precise character placement and check existing glyph and Unicode mappings. Text extraction preserves drawing order and coordinates for downstream processing.

## Why rzpdf?

**Work with existing PDFs, not just new ones.** Creation, parsing, page operations, and saving share a document model, so a PDF need not be only the final output.

**Keep the core separate from rendering.** The PDF core does not depend on Skia; add the renderer when you need previews or image output.

**Make edits deliberately.** Higher-level editing APIs use transactions, revision checks, and shared-resource isolation to help prevent an edit from affecting other pages unexpectedly.

**Know what is supported.** Regression tests, independent-tool checks, and visual checks against fixed samples define the supported scope. An available API does not imply universal PDF compatibility.

## Quick start

For now, start by running the examples from source. Rust 1.92 or newer is required. The first build uses native dependencies including FreeType and JPEG; building the renderer also involves Skia and may take some time.

```bash
git clone https://github.com/rockyzhengwu/rzpdf.git
cd rzpdf

# Create a two-page PDF with text, images, and metadata
cargo run -p pdf --locked --example create_document -- created.pdf

# Extract text in drawing order
cargo run -p pdf --locked --example extract_text -- created.pdf
```

### Create a PDF in Rust

```rust
use pdf::{
    document::{PdfDocument, resource_manager::StandardFont},
    error::PdfResult,
    page::content_builder::ContentBuilder,
};

fn main() -> PdfResult<()> {
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;

    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;

    let mut content = ContentBuilder::new();
    content.begin_text();
    content.try_set_font(&font, 24.0)?;
    content.move_text(48.0, 720.0);
    content.show_encoded_text(&StandardFont::Helvetica.encode_text("Hello, PDF!")?)?;
    content.end_text();

    doc.replace_page_content_builder(0, &content)?;
    doc.save_to_file("hello.pdf")
}
```

Page coordinates use PDF user space, with the origin at the lower left by default. Text must be encoded for the selected font; Chinese UTF-8 cannot be used directly as PDF character codes. See [create_cjk.rs](pdf/examples/create_cjk.rs) for a Chinese text example.

### More workflows

```bash
# Merge documents
cargo run -p pdf --locked --example merge_documents -- merged.pdf first.pdf second.pdf

# Select pages in a specified order (zero-based indexes)
cargo run -p pdf --locked --example extract_pages -- selected.pdf input.pdf 3 0

# Render pages as PNGs (target files in the output directory must not exist)
cargo run -p render --locked --example render_document -- input.pdf preview 144 144
```

The creation, merge, and page-selection examples replace an existing regular output file at the specified path. Use a fresh output path when you need to keep an existing file. More examples are available in [pdf/examples](pdf/examples) and [render/examples](render/examples).

## Current scope

rzpdf is still in early development and APIs may change. Engineering checks cover documented workflows and fixed input matrices, not universal PDF compatibility. No version has been formally released; validate the library against your own documents before production use.

Document interactions include [hierarchical navigation](docs/navigation.md), [common annotations](docs/annotations.md), attachments, and [AcroForm fields](docs/forms.md). Forms support multiple widgets, text, checkboxes, radio buttons, choices, and pushbuttons, with explicit embedded-font Unicode appearances. [Page relationship operations](docs/page-relationships.md) preserve supported navigation, annotations and fields with explicit conflict and deletion policies. Actions are preserved, not executed; the topic guides define supported editing and transfer limits.

The [advanced saving APIs](docs/advanced-saving.md) provide bounded incremental saves, AES-256 R6 protection, and external detached CMS approval signing. Authenticated R6 inputs can preserve their protection during incremental saves; [the support matrix](docs/releases/v0.11-support.md) defines version and input limits. [Offline verification](docs/signature-verification.md) checks historical CMS signatures using bounded RSA, RSA-PSS and P-256 ECDSA profiles. [Controlled workflows](docs/signed-workflows.md) add hierarchical signature fields, Unicode appearances and a bounded set of form/comment updates on unencrypted PDF 2.0, subject to DocMDP/FieldMDP constraints. Encrypted signing and arbitrary signed-document editing remain unsupported. Cryptographic verification does not establish certificate trust.

[Image workflows](docs/image-codecs.md) include typed inline images, source-aware edits, and in-process JPEG 2000 and JBIG2 decoding. The [image input matrix](docs/releases/v0.6-matrix.md) defines the bounded baseline: JPX alpha/ICC and symbol-coded JBIG2 scans are not supported. Original encoded images and shared JBIG2 Globals can be preserved without re-encoding.

Graphics include [patterns](docs/patterns.md), tensor-patch shading, [RGB transparency groups and soft masks](docs/transparency.md), TrueType text clipping, and [explicit ICC colour conversion](docs/color-management.md). The [graphics support matrix](docs/releases/v0.8-support.md) distinguishes tested display workflows from unsupported print-production and font combinations.

Text extraction does not reconstruct general reading order or paragraphs. Chinese support does not include complete font shaping, and rendering does not cover every PDF feature. Full annotation/form support, secure redaction and OCR remain outside the scope. Incremental saves retain historical bytes; covering text or removing a drawing operation is not secure erasure.

For input limits, supported fonts and images, and verified platforms, see the [v0.1 support matrix](docs/releases/v0.1-support.md), [v0.2 editing support matrix](docs/releases/v0.2-support.md), and [v0.7 font matrix](docs/releases/v0.7-font-matrix.md). Unknown unembedded fonts are explicit missing-font cases; declared aliases are substitutes, not exact reproduction. Font checks cover fixed Type1, CFF, OpenType/CFF, TrueType and bounded Type3 combinations, not arbitrary fonts or complex shaping.

## Documentation and development

- Guides: [text extraction](docs/text-extraction.md) · [glyph runs](docs/glyph-runs.md) · [Type3 fonts](docs/type3-fonts.md) · [image extraction](docs/image-extraction.md) · [image codecs](docs/image-codecs.md) · [content editing](docs/content-editing.md) · [advanced saving](docs/advanced-saving.md) · [navigation](docs/navigation.md) · [annotations](docs/annotations.md) · [forms](docs/forms.md)
- Design: [architecture](docs/architecture.md) · [cache and lifecycle](docs/cache-lifecycle.md)
- WebAssembly development: [specification](docs/webassembly/spec.md) · [implementation plan](docs/webassembly/plan.md) · [acceptance matrix](docs/webassembly/acceptance.md) · [target build guide](docs/webassembly/build.md). The P0 Chromium demo passes 19 pages with exact native pixel matches; the stable API and other browsers remain unverified.
- Releases: [roadmap](ROADMAP.md) · [interaction scope](docs/releases/v0.4.md) · [advanced saving scope](docs/releases/v0.5-support.md) · [changelog](CHANGELOG.md)
- Development and verification: [tasks and implementation history](TODO.md) · [testing](docs/testing.md) · [v0.1 acceptance](docs/releases/v0.1-acceptance.md) · [v0.2 acceptance](docs/releases/v0.2-acceptance.md) · [v0.3 acceptance](docs/releases/v0.3-acceptance.md) · [v0.4 acceptance](docs/releases/v0.4-acceptance.md) · [v0.5 acceptance](docs/releases/v0.5-acceptance.md) · [v0.6 acceptance](docs/releases/v0.6-acceptance.md)

The workspace has three crates: `pdf` provides the core, `render` provides the Skia backend, and `font-data` supplies bundled fonts and mapping data.

Run the regression tests with:

```bash
cargo test --workspace --locked
```

Issues with a minimal reproduction and expected result are welcome. Please include relevant tests with code contributions, and confirm that any contributed font or PDF samples can be redistributed.

## License

Project-owned code is licensed under the [GNU Affero General Public License v3.0 only](LICENSE) (`AGPL-3.0-only`). Bundled fonts, font metrics, and CMaps retain their separate licenses; see the [third-party data notice](font-data/NOTICE.md). Dependencies also retain their licenses; see the [security dependency review](docs/releases/v0.5-dependencies.md). The repository's root license alone is not sufficient for redistribution. See the [license transition record](docs/licensing.md) for scope and historical versions.

---

Detailed API notes and development status from the previous README are preserved in the [historical archive](docs/readme-history.md). Use the linked topic guides for current information.

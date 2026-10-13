# rzpdf

**Build, read, and work with PDFs in Rust.**

rzpdf is an evolving PDF toolkit that brings document creation, content extraction, page organization, editing, and rendering into one object model.

It is designed for Rust applications that need to generate PDFs, extract content, or work with pages and document resources.

## From creation to editing

- **Create documents:** Set page sizes, draw text, paths, and images, and write Info or XMP metadata.
- **Read and extract:** Parse documents, inspect page geometry, extract text, images, and drawing positions, and review parser diagnostics.
- **Organize pages:** Add, remove, copy, reorder, rotate, and crop pages; import pages across documents, merge files, or select pages in a specified order.
- **Open and save:** Load from a file or bytes and save to bytes, a writer, or a file, with options to preserve encoded streams or compress unencoded streams.
- **Render and edit:** Use a separate Skia renderer, or edit text, images, and paths within the documented font and source limits, including targeted edits in one-level, non-Group Forms.

Chinese text creation supports embedded static TrueType fonts. Standard Western fonts have explicit text-encoding APIs. Text extraction preserves drawing order and coordinates for downstream processing.

## Why rzpdf?

**Work with existing PDFs, not just new ones.** Creation, parsing, page operations, and saving share a document model, so a PDF need not be only the final output.

**Keep the core separate from rendering.** The PDF core does not depend on Skia; add the renderer when you need previews or image output.

**Make edits deliberately.** Higher-level editing APIs use transactions, revision checks, and shared-resource isolation to help prevent an edit from affecting other pages unexpectedly.

**Know what is supported.** Regression tests, independent-tool checks, and visual checks against fixed samples define the supported scope. An available API does not imply universal PDF compatibility.

## Quick start

For now, start by running the examples from source. You need a toolchain that supports Rust 2024 edition. The first build uses native dependencies including FreeType and JPEG; building the renderer also involves Skia and may take some time.

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

rzpdf is still in early development. The v0.1 core workflows and the limited v0.2 content-editing scope have passed engineering acceptance, but neither version has been formally released, and APIs may change. Validate the library against your own documents before production use.

Text extraction does not reconstruct general reading order or paragraphs. Chinese support does not include complete font shaping, and rendering does not cover every PDF feature. Full annotation and form support, incremental saving, signatures, secure redaction, and OCR are outside the current scope. Covering text or removing a drawing operation is not secure erasure.

For input limits, supported fonts and images, and verified platforms, see the [v0.1 support matrix](docs/releases/v0.1-support.md) and [v0.2 editing support matrix](docs/releases/v0.2-support.md). The [v0.3 plan](docs/releases/v0.3.md) describes future font and compatibility work; it is not an accepted feature set.

## Documentation and development

- Guides: [text extraction](docs/text-extraction.md) · [image extraction](docs/image-extraction.md) · [content editing](docs/content-editing.md)
- Design: [architecture](docs/architecture.md) · [cache and lifecycle](docs/cache-lifecycle.md)
- Releases: [roadmap](ROADMAP.md) · [changelog](CHANGELOG.md)
- Development and verification: [tasks and implementation history](TODO.md) · [testing](docs/testing.md) · [v0.1 acceptance](docs/releases/v0.1-acceptance.md) · [v0.2 acceptance](docs/releases/v0.2-acceptance.md)

The workspace has three crates: `pdf` provides the core, `render` provides the Skia backend, and `font-data` supplies bundled fonts and mapping data.

Run the regression tests with:

```bash
cargo test --workspace --locked
```

Issues with a minimal reproduction and expected result are welcome. Please include relevant tests with code contributions, and confirm that any contributed font or PDF samples can be redistributed.

## License

The project is licensed under [GNU GPL v3](LICENSE). Bundled fonts, font metrics, and CMaps have separate provenance and license requirements; see the [third-party data notice](font-data/NOTICE.md). The repository's root license alone is not sufficient for redistribution.

---

Detailed API notes and development status from the previous README are preserved in the [historical archive](docs/readme-history.md). Use the linked topic guides for current information.

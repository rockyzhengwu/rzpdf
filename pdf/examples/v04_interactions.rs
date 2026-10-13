//! Generate a small document for independent v0.4 interaction checks.
//! Usage: cargo run -p pdf --example v04_interactions -- OUTPUT.pdf

use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
};

fn main() -> PdfResult<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("output path required".into()))?;
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;
    doc.add_page(1)?;
    doc.add_flat_bookmark("Start", 0)?;
    doc.add_flat_bookmark("Second page", 1)?;
    doc.set_named_destination("chapter-two", 1)?;
    doc.add_page_link(0, [60.0, 700.0, 240.0, 725.0], 1)?;
    doc.add_named_link(0, [260.0, 700.0, 440.0, 725.0], "chapter-two")?;
    doc.add_uri_link(0, [60.0, 735.0, 240.0, 760.0], "https://example.org")?;
    doc.add_square_annotation(0, [45.0, 480.0, 280.0, 550.0], [1.0, 0.0, 0.0])?;
    doc.add_text_field(0, [60.0, 640.0, 300.0, 670.0], "name", "Alice")?;
    doc.add_checkbox_field(0, [60.0, 580.0, 90.0, 610.0], "confirmed", true)?;
    doc.add_attachment("note.txt", b"v0.4 fixture\n".to_vec())?;
    doc.add_attachment("说明.txt", b"unicode filename fixture\n".to_vec())?;
    doc.save_to_file(path)
}

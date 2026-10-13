//! Inspect the supported v0.4 interaction subset of an existing PDF.
//! Usage: cargo run -p pdf --example v04_inspect -- INPUT.pdf

use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
};

fn main() -> PdfResult<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("input path required".into()))?;
    let doc = PdfDocument::open(path)?;
    for bookmark in doc.flat_bookmarks()? {
        println!(
            "bookmark {:?} -> page {}",
            bookmark.title, bookmark.page_index
        );
    }
    for (name, page) in doc.named_destinations()? {
        println!("destination {name:?} -> page {page}");
    }
    for page in 0..doc.try_page_count()? {
        let page = u32::try_from(page).map_err(|_| PdfError::ResourceLimit("page index".into()))?;
        for link in doc.page_links(page)? {
            println!("page {page} link {:?}", link.target);
        }
        for square in doc.page_square_annotations(page)? {
            println!("page {page} Square {:?} RGB {:?}",square.rect,square.color);
        }
    }
    for name in doc.attachment_names()? {
        let data = doc
            .attachment_data(&name)?
            .ok_or_else(|| PdfError::DocumentError("attachment vanished".into()))?;
        println!("attachment {name:?}: {} bytes", data.len());
    }
    for field in doc.form_fields()? {
        println!("field {:?}: {:?}", field.name, field.value);
    }
    Ok(())
}

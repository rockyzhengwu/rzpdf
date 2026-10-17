//! Strictly read and modify an independently generated navigation document.
use pdf::{
    document::{
        PdfDocument, destination::DestinationView, links::LinkTarget, outlines::OutlineTarget,
    },
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected input PDF".into()))?;
    let output = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected output PDF".into()))?;
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    assert_eq!(doc.page_label(0)?, "pre-iv");
    assert_eq!(doc.page_label(1)?, "章-AA");
    let targets = doc.named_destination_views()?;
    assert_eq!(targets.len(), 2);
    assert_eq!(
        targets[0].1.view,
        DestinationView::Xyz {
            left: None,
            top: Some(150.),
            zoom: Some(1.25)
        }
    );
    assert_eq!(
        doc.page_links(0)?[0].target,
        LinkTarget::Named("alpha".into())
    );
    let items = doc.outline_items()?;
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].target, OutlineTarget::Named("alpha".into()));
    doc.rename_named_destination("alpha", "renamed")?;
    doc.move_outline(&items[1].handle, None, 0)?;
    doc.save_to_file(output)
}

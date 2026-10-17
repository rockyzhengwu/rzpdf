//! Structural navigation corpus; no fonts, downloads or external runtime needed.
use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        links::{LinkTarget, PageLink},
        outlines::OutlineTarget,
        page_labels::{PageLabelRange, PageLabelStyle},
    },
    error::{PdfError, PdfResult},
    page::page_builder::PageBuilder,
};

fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected output PDF".into()))?;
    let doc = PdfDocument::new()?;
    for (i, color) in ["1 0 0", "0 1 0", "0 0 1"].into_iter().enumerate() {
        doc.add_page_with_builder(i as u32, PageBuilder::new().size(200., 200.))?;
        doc.replace_page_contents(
            i as u32,
            format!("q {color} rg 10 10 180 180 re f Q").into_bytes(),
        )?;
    }
    doc.set_page_label_ranges(&[PageLabelRange {
        start_index: 0,
        style: Some(PageLabelStyle::RomanLower),
        prefix: "前-".into(),
        start_number: 1,
    }])?;
    for i in 0..70 {
        doc.set_named_destination(&format!("entry-{i:03}"), (i % 3) as u32)?;
    }
    let target = doc.destination(
        1,
        DestinationView::Xyz {
            left: Some(25.),
            top: Some(180.),
            zoom: None,
        },
    )?;
    doc.set_named_destination_view("旧目标", &target)?;
    let root = doc.add_outline(None, "章节", &OutlineTarget::Named("旧目标".into()))?;
    let child_target = OutlineTarget::Destination(doc.destination(
        2,
        DestinationView::FitR {
            left: 10.,
            bottom: 10.,
            right: 190.,
            top: 190.,
        },
    )?);
    let child = doc.add_outline(Some(&root), "折叠节", &child_target)?;
    doc.add_outline(Some(&child), "子节", &OutlineTarget::Named("旧目标".into()))?;
    doc.update_outline(&child, "折叠节", &child_target, false)?;
    let rect = [20., 20., 80., 40.];
    doc.add_named_link(0, rect, "旧目标")?;
    doc.add_link(
        0,
        &PageLink {
            rect: [20., 50., 80., 70.],
            target: LinkTarget::Destination(target),
        },
    )?;
    let temporary = doc.add_link(
        0,
        &PageLink {
            rect,
            target: LinkTarget::Page(0),
        },
    )?;
    doc.update_page_link(
        0,
        &temporary,
        &PageLink {
            rect: [20., 80., 80., 100.],
            target: LinkTarget::Uri("https://example.org/".into()),
        },
    )?;
    doc.rename_named_destination("旧目标", "目标")?;
    doc.reorder_pages(&[2, 0, 1])?;
    doc.save_to_file(output)
}

//! Reproducible import -> copy -> retarget/delete -> reorder acceptance corpus.
use pdf::{
    document::{
        PdfDocument,
        acroform::FormValue,
        annotations::{Annotation, AnnotationKind},
        destination::DestinationView,
        links::{LinkTarget, PageLink},
        outlines::OutlineTarget,
        page_labels::{PageLabelRange, PageLabelStyle},
        page_transfer::{PageDeletionPolicy, PageTransferOptions},
    },
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let out = std::path::PathBuf::from(args.next().ok_or_else(|| {
        PdfError::DocumentError("expected new output directory [source.pdf]".into())
    })?);
    let input = args.next();
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    std::fs::create_dir(&out)?;
    let source = if let Some(input) = input {
        PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?
    } else {
        let doc = PdfDocument::new()?;
        for i in 0..2 {
            doc.add_page(i)?;
            doc.replace_page_contents(i, b"q Q".to_vec())?;
        }
        doc.add_text_field(0, [20., 40., 180., 70.], "person.city", "Shanghai")?;
        let city = doc.form_field_tree()?[1].handle.clone();
        doc.add_form_widget(&city, 1, [20., 40., 180., 70.])?;
        doc.add_checkbox_field(0, [20., 90., 44., 114.], "person.ok", true)?;
        let square = doc.add_annotation(
            0,
            &Annotation::new(AnnotationKind::Square, [50., 90., 130., 140.]),
        )?;
        let popup = doc.add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::Popup { open: false },
                [150., 90., 250., 140.],
            ),
        )?;
        doc.set_annotation_popup(&square, Some(&popup))?;
        let reply = doc.add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::Text {
                    icon: "Note".into(),
                    open: false,
                },
                [270., 90., 290., 110.],
            ),
        )?;
        doc.set_annotation_reply(&reply, Some(&square))?;
        doc.set_named_destination("start", 0)?;
        doc.add_link(
            1,
            &PageLink {
                rect: [20., 150., 130., 175.],
                target: LinkTarget::Named("start".into()),
            },
        )?;
        let root = doc.add_outline(None, "Start", &OutlineTarget::Named("start".into()))?;
        doc.add_outline(
            Some(&root),
            "Next",
            &OutlineTarget::Destination(doc.destination(1, DestinationView::FitH(Some(200.)))?),
        )?;
        doc.set_page_label_ranges(&[PageLabelRange {
            start_index: 0,
            style: Some(PageLabelStyle::RomanLower),
            prefix: "s-".into(),
            start_number: 3,
        }])?;
        doc
    };
    if source.try_page_count()? != 2 {
        return Err(PdfError::DocumentError(
            "acceptance source must contain two pages".into(),
        ));
    }
    std::fs::write(out.join("source.pdf"), source.save_to_bytes()?)?;
    let target = PdfDocument::new()?;
    target.add_page(0)?;
    target.import_pages_from_with_options(
        &source,
        &[0, 1],
        1,
        &PageTransferOptions {
            name_prefix: "imported".into(),
            include_navigation: true,
            ..Default::default()
        },
    )?;
    std::fs::write(out.join("imported.pdf"), target.save_to_bytes()?)?;
    target.copy_page_with_options(
        1,
        3,
        &PageTransferOptions {
            name_prefix: "copy".into(),
            ..Default::default()
        },
    )?;
    std::fs::write(out.join("copied.pdf"), target.save_to_bytes()?)?;
    target.delete_page_with_policy(1, PageDeletionPolicy::Retarget { page_index: 3 })?;
    target.reorder_pages(&[2, 0, 1])?;
    let reopened = PdfDocument::from_bytes_with_mode(target.save_to_bytes()?, ParseMode::Strict)?;
    std::fs::write(out.join("final.pdf"), reopened.save_to_bytes()?)?;
    // Imported font/DA resources must also work after a strict save/reopen.
    let city = reopened
        .form_fields()?
        .into_iter()
        .find(|f| matches!(f.value, FormValue::Text(_)))
        .ok_or_else(|| PdfError::DocumentError("expected a text field".into()))?;
    reopened.set_form_value(&city.name, FormValue::Text("Suzhou".into()))?;
    std::fs::write(out.join("refilled.pdf"), reopened.save_to_bytes()?)?;
    Ok(())
}

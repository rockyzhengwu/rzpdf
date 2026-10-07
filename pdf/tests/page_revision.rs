use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
};

#[test]
fn retained_page_reinterprets_content_after_edit_and_rollback() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut page = doc.get_page(0).unwrap();
    assert!(page.display_list().unwrap().objects().is_empty());
    let original = doc.stamp();
    doc.replace_page_contents(0, b"0 0 10 10 re f\n".to_vec())
        .unwrap();
    assert!(!doc.is_stamp_current(&original));
    assert_eq!(page.display_list().unwrap().objects().len(), 1);
    let committed = doc.stamp();
    let mut tentative = committed.clone();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.replace_page_contents(0, b"0 0 10 10 re f 20 20 10 10 re f\n".to_vec())?;
        tentative = doc.stamp();
        assert_eq!(page.display_list()?.objects().len(), 2);
        Err(PdfError::DocumentError("restore content".into()))
    });
    assert!(result.is_err());
    assert!(!doc.is_stamp_current(&committed));
    assert!(!doc.is_stamp_current(&tentative));
    assert_eq!(page.display_list().unwrap().objects().len(), 1);
    let restored = doc.stamp();
    doc.save_to_bytes().unwrap();
    assert!(doc.is_stamp_current(&restored));
}

#[test]
fn identical_revisions_in_distinct_documents_do_not_match() {
    use std::collections::HashSet;
    let first = PdfDocument::new().unwrap();
    let second = PdfDocument::new().unwrap();
    assert_eq!(first.revision(), second.revision());
    let stamp = first.stamp();
    assert!(first.is_stamp_current(&stamp));
    assert!(!second.is_stamp_current(&stamp));
    assert_eq!(
        HashSet::from([stamp.clone(), first.stamp(), second.stamp()]).len(),
        2
    );
}

#[test]
fn retained_page_refreshes_geometry_after_low_level_update_and_unwind() {
    use pdf::objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_number::PdfNumber,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let pages = doc.resolve_object(ObjectId::new(1, 0)).unwrap();
    let page_id = pages
        .as_dict()
        .unwrap()
        .get("Kids")
        .unwrap()
        .as_array()
        .unwrap()
        .get(0)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let mut page = doc.get_page(0).unwrap();
    let original_width = page.display_list().unwrap().page_width();
    let mut dict = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut box_array = PdfArray::default();
    for value in [0., 0., 200., 300.] {
        box_array.add_obj(PdfObject::PdfNumber(PdfNumber::new(value)));
    }
    dict.try_insert("MediaBox".into(), PdfObject::PdfArray(box_array))
        .unwrap();
    let mut tentative = doc.stamp();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|doc| {
                doc.update_object(page_id, PdfObject::PdfDict(dict))?;
                tentative = doc.stamp();
                let list = page.display_list()?;
                assert_eq!(list.page_width(), 200.);
                assert_eq!(list.page_height(), 300.);
                panic!("restore geometry");
            });
        }))
        .is_err()
    );
    assert!(!doc.is_stamp_current(&tentative));
    assert_eq!(page.display_list().unwrap().page_width(), original_width);
}

#[test]
fn source_cached_font_refreshes_after_edit_and_transaction_rollback() {
    use pdf::{
        document::resource_manager::StandardFont,
        objects::{PdfObject, pdf_name::PdfName},
        page::{display_list::DisplayList, page_object::PageObject},
        parser::syntax::ParseMode,
    };
    fn font_name(list: &DisplayList) -> &str {
        let PageObject::Text(text) = &list.objects()[0] else {
            panic!("expected text");
        };
        text.state.text_state.font.as_ref().unwrap().name()
    }
    let source = PdfDocument::new().unwrap();
    source.add_page(0).unwrap();
    let name = source
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    source
        .replace_page_contents(0, format!("BT /{name} 12 Tf (test) Tj ET\n").into_bytes())
        .unwrap();
    let doc = PdfDocument::from_bytes_with_mode(source.save_to_bytes().unwrap(), ParseMode::Strict)
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let original_list = page.display_list().unwrap();
    assert_eq!(font_name(&original_list), "Helvetica");
    let resources = page.resource().unwrap();
    let font_id = resources
        .get("Font")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(&name)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let original = doc.resolve_object(font_id).unwrap();
    let mut changed = original.clone().into_dict().unwrap();
    changed
        .try_insert(
            "BaseFont".into(),
            PdfObject::PdfName(PdfName::new("Courier".into())),
        )
        .unwrap();
    doc.update_object(font_id, PdfObject::PdfDict(changed))
        .unwrap();
    assert_eq!(font_name(&page.display_list().unwrap()), "Courier");
    assert_eq!(font_name(&original_list), "Helvetica");
    let commit = doc.commit_revision();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.update_object(font_id, original)?;
        assert_eq!(font_name(&page.display_list()?), "Helvetica");
        Err(PdfError::DocumentError("restore font".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.commit_revision(), commit);
    assert_eq!(font_name(&page.display_list().unwrap()), "Courier");
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        font_name(&reopened.get_page(0).unwrap().display_list().unwrap()),
        "Courier"
    );
}

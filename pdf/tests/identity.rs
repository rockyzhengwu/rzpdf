use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    objects::PdfObject,
};

#[test]
fn same_number_in_different_documents_is_not_the_same_handle() {
    let first = PdfDocument::new().unwrap();
    let second = PdfDocument::new().unwrap();
    let first_id = first.try_insert_object(PdfObject::PdfNull).unwrap();
    let second_id = second
        .try_insert_object(PdfObject::PdfDict(Default::default()))
        .unwrap();
    assert_eq!(first_id, second_id);
    let handle = first.object_handle(first_id).unwrap();
    assert_eq!(handle.document_identity(), &first.identity());
    assert_ne!(first.identity(), second.identity());
    let before = second.revision();
    assert!(second.resolve_handle(&handle).is_err());
    assert!(second.update_handle(&handle, PdfObject::PdfNull).is_err());
    assert!(second.delete_handle(&handle).is_err());
    assert!(
        second
            .resources()
            .register_handle(0, ResourceKind::Font, &handle)
            .is_err()
    );
    assert_eq!(second.revision(), before);
    assert!(
        second
            .resolve_object(second_id)
            .unwrap()
            .as_dict()
            .is_some()
    );
}

#[test]
fn local_handles_follow_updates_and_reject_deleted_targets() {
    let doc = PdfDocument::new().unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let handle = doc.object_handle(id).unwrap();
    let identity = doc.identity();
    doc.update_handle(&handle, PdfObject::PdfDict(Default::default()))
        .unwrap();
    assert!(doc.resolve_handle(&handle).unwrap().as_dict().is_some());
    assert_eq!(doc.identity(), identity);
    doc.delete_handle(&handle).unwrap();
    assert!(doc.resolve_handle(&handle).is_err());
    assert!(doc.object_handle(id).is_err());
}

#[test]
fn reopen_has_a_new_identity_even_with_identical_object_ids() {
    use std::collections::HashSet;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let handle = doc.object_handle(id).unwrap();
    let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    assert!(reopened.resolve_object(id).is_ok());
    assert!(reopened.resolve_handle(&handle).is_err());
    let identities = HashSet::from([doc.identity(), doc.identity(), reopened.identity()]);
    assert_eq!(identities.len(), 2);
    let kept = doc.identity();
    drop(doc);
    assert_ne!(PdfDocument::new().unwrap().identity(), kept);
}

#[test]
fn scoped_resource_registration_reuses_local_font() {
    use pdf::objects::{pdf_dict::PdfDict, pdf_name::PdfName};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut dict = PdfDict::default();
    for (key, value) in [
        ("Type", "Font"),
        ("Subtype", "Type1"),
        ("BaseFont", "Helvetica"),
    ] {
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
            .unwrap();
    }
    let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let handle = doc.object_handle(id).unwrap();
    let first = doc
        .resources()
        .register_handle(0, ResourceKind::Font, &handle)
        .unwrap();
    let second = doc
        .resources()
        .register_handle(0, ResourceKind::Font, &handle)
        .unwrap();
    assert_eq!(first, "F1");
    assert_eq!(second, first);
}

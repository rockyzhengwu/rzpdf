use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId},
};

#[test]
fn outer_transaction_commits_once_and_inner_rollback_does_not_commit() {
    let doc = PdfDocument::new().unwrap();
    let before = doc.commit_revision();
    doc.edit_transaction(|doc| {
        doc.add_page(0)?;
        let inner: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.add_page(1)?;
            Err(PdfError::DocumentError("inner rollback".into()))
        });
        assert!(inner.is_err());
        assert_eq!(doc.try_page_count()?, 1);
        doc.add_page(1)?;
        assert_eq!(doc.commit_revision(), before);
        Ok(())
    })
    .unwrap();
    assert_eq!(doc.commit_revision(), before + 1);
    let committed = doc.commit_revision();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.add_page(2)?;
        Err(PdfError::DocumentError("outer rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.commit_revision(), committed);
    assert_eq!(doc.try_page_count().unwrap(), 2);
}

#[test]
fn rolled_back_inner_edits_do_not_dirty_read_only_outer_transaction() {
    let doc = PdfDocument::new().unwrap();
    let before = doc.commit_revision();
    doc.edit_transaction(|doc| {
        let inner: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.add_page(0)?;
            Err(PdfError::DocumentError("discard all writes".into()))
        });
        assert!(inner.is_err());
        Ok(())
    })
    .unwrap();
    doc.edit_transaction(|_| Ok(())).unwrap();
    assert_eq!(doc.commit_revision(), before);
    doc.try_insert_object(PdfObject::PdfNull).unwrap();
    assert_eq!(doc.commit_revision(), before + 1);
}

#[test]
fn low_level_edits_advance_revision_but_reads_and_failed_writes_do_not() {
    let doc = PdfDocument::new().unwrap();
    let before = doc.revision();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let inserted = doc.revision();
    assert!(inserted > before);
    doc.resolve_object(id).unwrap();
    assert_eq!(doc.revision(), inserted);
    assert!(
        doc.update_object(ObjectId::new(999, 0), PdfObject::PdfNull)
            .is_err()
    );
    assert_eq!(doc.revision(), inserted);
    doc.update_object(id, PdfObject::PdfNull).unwrap();
    assert!(doc.revision() > inserted);
    let updated = doc.revision();
    doc.delete_object(id).unwrap();
    assert!(doc.revision() > updated);
}

#[test]
fn rollback_never_reuses_a_tentative_revision() {
    let doc = PdfDocument::new().unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let mut tentative = doc.revision();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.update_object(id, PdfObject::PdfDict(Default::default()))?;
        tentative = doc.revision();
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert!(doc.revision() > tentative);
    assert_eq!(doc.resolve_object(id).unwrap(), PdfObject::PdfNull);
    let before = doc.revision();
    doc.edit_transaction(|_| Ok(())).unwrap();
    assert_eq!(doc.revision(), before);
}

#[test]
fn page_edits_advance_revision_and_saving_is_read_only() {
    let doc = PdfDocument::new().unwrap();
    let before = doc.revision();
    doc.add_page(0).unwrap();
    let added = doc.revision();
    assert!(added > before);
    assert_eq!(doc.try_page_count().unwrap(), 1);
    doc.save_to_bytes().unwrap();
    assert_eq!(doc.revision(), added);
    doc.replace_page_contents(0, b"q Q\n".to_vec()).unwrap();
    assert!(doc.revision() > added);
}

#[test]
fn nested_rollback_and_unwind_invalidate_tentative_state() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let doc = PdfDocument::new().unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let mut inner_revision = 0;
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.update_object(id, PdfObject::PdfDict(Default::default()))?;
        let outer_revision = doc.revision();
        let inner: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.update_object(id, PdfObject::PdfNull)?;
            inner_revision = doc.revision();
            Err(PdfError::DocumentError("inner rollback".into()))
        });
        assert!(inner.is_err());
        assert!(doc.revision() > inner_revision);
        assert!(doc.revision() > outer_revision);
        assert!(doc.resolve_object(id)?.as_dict().is_some());
        Err(PdfError::DocumentError("outer rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.resolve_object(id).unwrap(), PdfObject::PdfNull);
    let mut unwind_revision = 0;
    let unwind_commit = doc.commit_revision();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|doc| {
                doc.update_object(id, PdfObject::PdfDict(Default::default()))?;
                unwind_revision = doc.revision();
                panic!("test rollback on unwind");
            });
        }))
        .is_err()
    );
    assert!(doc.revision() > unwind_revision);
    assert_eq!(doc.commit_revision(), unwind_commit);
    assert_eq!(doc.resolve_object(id).unwrap(), PdfObject::PdfNull);
}

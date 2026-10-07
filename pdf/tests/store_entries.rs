use pdf::document::store::{DocumentStore, StoreEntries, StoreEntry};
use pdf::objects::{PdfObject, object_id::ObjectId};

#[test]
fn public_store_view_preserves_queries_and_borrowed_iteration() {
    let mut store = DocumentStore::new(10);
    assert!(store.entries().is_empty());
    let live = store.try_insert_object(PdfObject::PdfNull).unwrap();
    let deleted = ObjectId::new(20, 0);
    store.delete_object(deleted).unwrap();

    let view: StoreEntries<'_> = store.entries();
    assert_eq!(view.len(), 2);
    assert!(!view.is_empty());
    assert!(view.capacity() >= view.len());
    assert!(view.contains_key(&live));
    assert!(matches!(
        view.get(&live),
        Some(StoreEntry::Upsert(PdfObject::PdfNull))
    ));
    assert!(matches!(view.get(&deleted), Some(StoreEntry::Deleted)));
    assert!(view.get(&ObjectId::new(99, 0)).is_none());

    // Returned references/iterators borrow the store, not a temporary view.
    let value = store.entries().get(&live).unwrap();
    let keys = store.entries().keys();
    let values = store.entries().values();
    let pairs = store.entries().iter();
    assert!(std::ptr::eq(value, store.entry(live).unwrap()));
    assert_eq!(keys.count(), 2);
    assert_eq!(
        values
            .filter(|value| matches!(value, StoreEntry::Deleted))
            .count(),
        1
    );
    for (id, entry) in pairs {
        assert!(std::ptr::eq(entry, store.entry(*id).unwrap()));
    }

    // Releasing views permits subsequent edits; no cloned table is retained.
    store.update_object(deleted, PdfObject::PdfNull).unwrap();
    assert!(matches!(
        store.entries().get(&deleted),
        Some(StoreEntry::Upsert(PdfObject::PdfNull))
    ));
}

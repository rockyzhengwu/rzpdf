use crate::objects::object_id::ObjectId;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

/// Process-local identity of one open document. Not serialized into the PDF.
/// Clones retain the allocation, so a live identity cannot alias a later document.
#[derive(Clone, Debug)]
pub struct DocumentIdentity(Arc<()>);

impl DocumentIdentity {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl PartialEq for DocumentIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for DocumentIdentity {}
impl Hash for DocumentIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// Cache provenance for one visible document state, not a stored object snapshot.
/// Use together with the object/page address and operation-specific options.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentStamp {
    pub(crate) document: DocumentIdentity,
    pub(crate) revision: u64,
}

impl DocumentStamp {
    pub fn document_identity(&self) -> &DocumentIdentity {
        &self.document
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// A document-scoped PDF object address, not an immutable object snapshot.
/// It follows updates to the same ID. IDs allocated inside a transaction must
/// not escape before commit: rollback may allow an ID to be allocated again.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectHandle {
    pub(crate) document: DocumentIdentity,
    pub(crate) id: ObjectId,
}

impl ObjectHandle {
    pub fn object_id(&self) -> ObjectId {
        self.id
    }
    pub fn document_identity(&self) -> &DocumentIdentity {
        &self.document
    }
}

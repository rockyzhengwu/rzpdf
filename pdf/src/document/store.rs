use std::collections::HashMap;

use crate::objects::{PdfObject, object_id::ObjectId};

#[derive(Debug, Clone)]
pub enum StoreEntry {
    Upsert(PdfObject),
    Deleted,
}

#[derive(Debug)]
pub struct DocumentStore {
    entries: HashMap<ObjectId, StoreEntry>,
    next_obj_num: u32,
}

impl DocumentStore {
    pub fn new(next_obj_num: u32) -> Self {
        Self {
            entries: HashMap::new(),
            next_obj_num,
        }
    }

    pub fn insert_object(&mut self, object: PdfObject) -> ObjectId {
        let object_id = ObjectId::new(self.next_obj_num, 0);
        self.next_obj_num += 1;
        self.entries.insert(object_id, StoreEntry::Upsert(object));
        object_id
    }

    pub fn update_object(&mut self, object_id: ObjectId, object: PdfObject) {
        self.entries.insert(object_id, StoreEntry::Upsert(object));
    }

    pub fn delete_object(&mut self, object_id: ObjectId) {
        self.entries.insert(object_id, StoreEntry::Deleted);
    }

    pub fn entry(&self, object_id: ObjectId) -> Option<&StoreEntry> {
        self.entries.get(&object_id)
    }

    pub fn entries(&self) -> &HashMap<ObjectId, StoreEntry> {
        &self.entries
    }
}

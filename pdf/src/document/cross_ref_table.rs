use std::collections::HashMap;

use crate::objects::{object_id::ObjectId, pdf_dict::PdfDict};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectState {
    Normal,
    Free,
    Compressed,
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    objnum: u32,
    offset: u64,
    gennum: u32,
    state: ObjectState,
}

impl ObjectInfo {
    pub fn new(objnum: u32, offset: u64, gennum: u32, state: ObjectState) -> Self {
        ObjectInfo {
            objnum,
            offset,
            gennum,
            state,
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }
    pub fn gennum(&self) -> u32 {
        self.gennum
    }

    pub fn state(&self) -> &ObjectState {
        &self.state
    }

    pub fn object_id(&self) -> ObjectId {
        ObjectId::new(self.objnum, self.gennum)
    }
}

#[derive(Debug, Default, Clone)]
pub struct CrossRefTable {
    objects: HashMap<u32, ObjectInfo>,
    trailer: PdfDict,
}

impl CrossRefTable {
    pub fn new_empty() -> Self {
        Self {
            objects: HashMap::new(),
            trailer: PdfDict::default(),
        }
    }
    pub fn new(objects: HashMap<u32, ObjectInfo>, trailer: PdfDict) -> Self {
        Self { objects, trailer }
    }

    pub fn trailer(&self) -> &PdfDict {
        &self.trailer
    }

    /// Transfer parsed document roots into document state without cloning them.
    pub(crate) fn take_trailer(&mut self) -> PdfDict {
        std::mem::take(&mut self.trailer)
    }

    pub fn merge(&mut self, other: CrossRefTable) {
        for (key, obj) in other.objects {
            self.objects.insert(key, obj);
        }
    }

    pub fn merge_older(&mut self, other: CrossRefTable) {
        for (key, object) in other.objects {
            self.objects.entry(key).or_insert(object);
        }
    }

    pub fn lookup(&self, objnum: &u32) -> Option<&ObjectInfo> {
        self.objects.get(objnum)
    }

    pub fn lookup_id(&self, object_id: ObjectId) -> Option<&ObjectInfo> {
        self.lookup(&object_id.obj_num()).filter(|info| {
            if matches!(info.state(), ObjectState::Compressed) {
                // For compressed objects this xref field is an object-stream index,
                // not a generation number. Compressed objects have generation zero.
                object_id.gen_num() == 0
            } else {
                info.gennum() == object_id.gen_num()
            }
        })
    }

    pub fn objects(&self) -> &HashMap<u32, ObjectInfo> {
        &self.objects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_checks_generation_without_using_compressed_index_as_generation() {
        let table = CrossRefTable::new(
            HashMap::from([
                (1, ObjectInfo::new(1, 100, 3, ObjectState::Normal)),
                (2, ObjectInfo::new(2, 10, 7, ObjectState::Compressed)),
                (3, ObjectInfo::new(3, 0, 4, ObjectState::Free)),
            ]),
            PdfDict::default(),
        );
        assert!(table.lookup_id(ObjectId::new(1, 3)).is_some());
        assert!(table.lookup_id(ObjectId::new(1, 0)).is_none());
        assert!(table.lookup_id(ObjectId::new(2, 0)).is_some());
        for generation in [1, 7, u32::MAX] {
            assert!(table.lookup_id(ObjectId::new(2, generation)).is_none());
        }
        assert!(matches!(
            table.lookup_id(ObjectId::new(3, 4)).unwrap().state(),
            ObjectState::Free
        ));
        assert!(table.lookup_id(ObjectId::new(3, 0)).is_none());
        assert!(table.lookup_id(ObjectId::new(999, 0)).is_none());
    }
}

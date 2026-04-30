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

    pub fn merge(&mut self, other: CrossRefTable) {
        for (key, obj) in other.objects {
            self.objects.insert(key, obj);
        }
    }

    pub fn lookup(&self, objnum: &u32) -> Option<&ObjectInfo> {
        self.objects.get(objnum)
    }

    pub fn lookup_id(&self, object_id: ObjectId) -> Option<&ObjectInfo> {
        self.lookup(&object_id.obj_num()).filter(|info| {
            matches!(info.state(), ObjectState::Compressed) || info.gennum() == object_id.gen_num()
        })
    }

    pub fn objects(&self) -> &HashMap<u32, ObjectInfo> {
        &self.objects
    }
}

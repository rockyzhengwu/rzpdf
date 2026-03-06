use std::collections::HashMap;

use crate::objects::pdf_dict::PdfDict;

#[derive(Debug)]
pub enum ObjectState {
    Normal,
    Free,
    Compressed,
}

#[derive(Debug)]
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
}

#[derive(Debug, Default)]
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
    pub fn objects(&self) -> &HashMap<u32, ObjectInfo> {
        &self.objects
    }
}

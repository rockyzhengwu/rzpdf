use crate::objects::object_id::ObjectId;

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct PdfReference {
    id: ObjectId,
}

impl PdfReference {
    pub fn new(objnum: u32, gennum: u32) -> Self {
        Self {
            id: ObjectId::new(objnum, gennum),
        }
    }

    pub fn from_id(id: ObjectId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn objnum(&self) -> u32 {
        self.id.obj_num()
    }

    pub fn gennum(&self) -> u32 {
        self.id.gen_num()
    }
}

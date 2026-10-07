use crate::objects::PdfObject;

#[derive(Debug, Clone, PartialEq)]
pub struct PdfIndirect {
    objnum: u32,
    gennum: u16,
    obj: PdfObject,
}

impl PdfIndirect {
    pub fn new(objnum: u32, gennum: u16, obj: PdfObject) -> Self {
        Self {
            objnum,
            gennum,
            obj,
        }
    }
    pub fn obj(&self) -> &PdfObject {
        &self.obj
    }
    pub fn id(&self) -> crate::objects::object_id::ObjectId {
        crate::objects::object_id::ObjectId::new(self.objnum, u32::from(self.gennum))
    }
    pub fn to_obj(self) -> PdfObject {
        self.obj
    }
}

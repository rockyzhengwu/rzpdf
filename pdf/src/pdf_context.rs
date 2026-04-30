use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::store::{DocumentStore, StoreEntry};
use crate::document::cross_ref_table::ObjectState;
use crate::objects::object_streams::ObjectStreams;
use crate::{
    document::cross_ref_table::CrossRefTable,
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
    parser::parser::PdfParser,
};

#[derive(Debug)]
pub struct PDFContext<R: Seek + Read> {
    parser: RefCell<PdfParser<R>>,
    cross_ref_table: CrossRefTable,
    indirect_objects: HashMap<ObjectId, OnceCell<PdfObject>>,
    store: RefCell<DocumentStore>,
}

impl<R: Seek + Read> PDFContext<R> {
    pub fn try_new(parser: PdfParser<R>, cross_ref_table: CrossRefTable) -> PdfResult<Self> {
        let mut indirect_objects = HashMap::new();
        for (objnum, objinfo) in cross_ref_table.objects() {
            if matches!(objinfo.state(), ObjectState::Free) {
                continue;
            }
            indirect_objects.insert(ObjectId::new(*objnum, objinfo.gennum()), OnceCell::new());
        }
        let next_obj_num = cross_ref_table
            .objects()
            .keys()
            .max()
            .map(|v| v + 1)
            .unwrap_or(1);
        Ok(PDFContext {
            parser: RefCell::new(parser),
            cross_ref_table,
            indirect_objects,
            store: RefCell::new(DocumentStore::new(next_obj_num)),
        })
    }

    pub fn parse_indirect_objects(&mut self) -> PdfResult<()> {
        for object_id in self.indirect_objects.keys() {
            let object_ref = PdfObject::PdfReference(PdfReference::from_id(*object_id));
            let _ = self.resolve(&object_ref)?;
        }
        Ok(())
    }

    pub fn get_root(&self) -> PdfResult<&PdfDict> {
        let root = self
            .cross_ref_table
            .trailer()
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve(root)?;
        let root = root
            .as_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn get_root_owned(&self) -> PdfResult<PdfDict> {
        let root = self
            .cross_ref_table
            .trailer()
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve_owned(root)?;
        let root = root
            .into_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn trailer(&self) -> &PdfDict {
        self.cross_ref_table.trailer()
    }

    pub fn object_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<_> = self.indirect_objects.keys().copied().collect();
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn serializable_object_ids(&self) -> Vec<ObjectId> {
        let mut ids = self.object_ids();
        for object_id in self.store.borrow().entries().keys() {
            if !ids.contains(object_id) {
                ids.push(*object_id);
            }
        }
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn materialize_all_objects(&self) -> PdfResult<()> {
        for object_id in self.object_ids() {
            self.resolve_object_id(object_id)?;
        }
        Ok(())
    }

    pub fn resolve<'a>(&'a self, obj: &'a PdfObject) -> PdfResult<&'a PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_id(reference.id()),
            _ => Ok(obj),
        }
    }

    pub fn resolve_owned(&self, obj: &PdfObject) -> PdfResult<PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_owned(reference.id()),
            _ => Ok(obj.clone()),
        }
    }

    pub fn resolve_object_id(&self, object_id: ObjectId) -> PdfResult<&PdfObject> {
        self.load_indirect_object(object_id)
    }

    pub fn resolve_object_owned(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        match self.store.borrow().entry(object_id).cloned() {
            Some(StoreEntry::Upsert(object)) => Ok(object),
            Some(StoreEntry::Deleted) => Err(PdfError::ObjectError(format!(
                "object has been deleted: {:?}",
                object_id
            ))),
            None => Ok(self.resolve_object_id(object_id)?.clone()),
        }
    }

    pub fn object_for_write(&self, object_id: ObjectId) -> PdfResult<Option<PdfObject>> {
        if let Some(entry) = self.store.borrow().entry(object_id).cloned() {
            return match entry {
                StoreEntry::Upsert(object) => Ok(Some(object)),
                StoreEntry::Deleted => Ok(None),
            };
        }

        if !self.indirect_objects.contains_key(&object_id) {
            return Err(PdfError::ObjectError(format!(
                "object does not exist in document store: {:?}",
                object_id
            )));
        }
        Ok(Some(self.resolve_object_id(object_id)?.clone()))
    }

    pub fn insert_object(&self, object: PdfObject) -> ObjectId {
        self.store.borrow_mut().insert_object(object)
    }

    pub fn update_object(&self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        if !self.indirect_objects.contains_key(&object_id)
            && self.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot update missing object: {:?}",
                object_id
            )));
        }
        self.store.borrow_mut().update_object(object_id, object);
        Ok(())
    }

    pub fn delete_object(&self, object_id: ObjectId) -> PdfResult<()> {
        if !self.indirect_objects.contains_key(&object_id)
            && self.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot delete missing object: {:?}",
                object_id
            )));
        }
        self.store.borrow_mut().delete_object(object_id);
        Ok(())
    }

    fn load_indirect_object<'a>(&'a self, object_id: ObjectId) -> PdfResult<&'a PdfObject> {
        let Some(cell) = self.indirect_objects.get(&object_id) else {
            return Err(PdfError::ObjectError(format!(
                "indirect object does not exist: {:?}",
                object_id
            )));
        };

        if let Some(obj) = cell.get() {
            return Ok(obj);
        }

        let objinfo = self
            .cross_ref_table
            .lookup_id(object_id)
            .ok_or(PdfError::ObjectError(format!(
                "xref entry does not exist: {:?}",
                object_id
            )))?;

        let object = match objinfo.state() {
            ObjectState::Normal => self.parse_indirect_object(objinfo.offset()),
            ObjectState::Compressed => {
                let stream_ref = ObjectId::new(objinfo.offset() as u32, 0);
                self.load_object_stream(stream_ref)?;
                self.indirect_objects
                    .get(&object_id)
                    .and_then(OnceCell::get)
                    .cloned()
                    .ok_or(PdfError::ObjectError(format!(
                        "compressed object was not populated from object stream: {:?}",
                        object_id
                    )))
            }
            ObjectState::Free => Err(PdfError::ObjectError(format!(
                "attempted to resolve free object: {:?}",
                object_id
            ))),
        }?;

        let _ = cell.set(object);
        cell.get().ok_or(PdfError::ObjectError(format!(
            "failed to cache resolved object: {:?}",
            object_id
        )))
    }

    fn parse_indirect_object(&self, offset: u64) -> PdfResult<PdfObject> {
        let obj = self.parser.borrow_mut().parse_indirect_obj(offset)?;
        self.populate_object_stream_entries(&obj)?;
        Ok(obj)
    }

    fn populate_object_stream_entries(&self, obj: &PdfObject) -> PdfResult<()> {
        let Some(stream) = obj.as_stream() else {
            return Ok(());
        };
        let Some(st) = stream.dict().get("Type") else {
            return Ok(());
        };
        let Some(name) = st.as_name() else {
            return Ok(());
        };
        if name.name() != "ObjStm" {
            return Ok(());
        }

        let data = stream.decode_data(self)?;
        let mut object_streams = ObjectStreams::try_new(stream.dict(), data)?;
        for (objnum, obj) in object_streams.parse_objects()? {
            let object_id = ObjectId::new(objnum, 0);
            if let Some(cell) = self.indirect_objects.get(&object_id) {
                let _ = cell.set(obj);
            }
        }
        Ok(())
    }

    fn load_object_stream(&self, stream_ref: ObjectId) -> PdfResult<()> {
        let object_stream = self.load_indirect_object(stream_ref)?;
        let stream = object_stream.as_stream().ok_or(PdfError::ObjectError(format!(
            "object stream reference does not point to a stream: {:?}",
            stream_ref
        )))?;
        self.populate_object_stream_entries(&PdfObject::PdfStream(stream.clone()))
    }
}

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::ObjectState;
use crate::objects::object_streams::ObjectStreams;
use crate::objects::pdf_reference::PdfReference;
use crate::{
    document::cross_ref_table::CrossRefTable,
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
    parser::parser::PdfParser,
};

#[derive(Debug)]
pub struct PDFContext<R: Seek + Read> {
    parser: RefCell<PdfParser<R>>,
    cross_ref_table: CrossRefTable,
    indirect_objects: HashMap<PdfReference, PdfObject>,
}

impl<R: Seek + Read> PDFContext<R> {
    pub fn try_new(parser: PdfParser<R>, cross_ref_table: CrossRefTable) -> PdfResult<Self> {
        let indirect_objects = HashMap::new();
        Ok(PDFContext {
            parser: RefCell::new(parser),
            cross_ref_table,
            indirect_objects,
        })
    }

    pub fn parse_indirect_objects(&mut self) -> PdfResult<()> {
        for (objnum, objinfo) in self.cross_ref_table.objects() {
            match objinfo.state() {
                ObjectState::Compressed => {
                    continue;
                }
                ObjectState::Free => {
                    continue;
                }
                ObjectState::Normal => {}
            }
            let offset = objinfo.offset();
            let gennum = objinfo.gennum();
            let pdfref = PdfReference::new(objnum.to_owned(), gennum);
            let obj = self.parser.borrow_mut().parse_indirect_obj(offset)?;
            match &obj {
                PdfObject::PdfStream(stream) => {
                    if let Some(st) = stream.dict().get("Type") {
                        if st.as_name().unwrap().name() == "ObjStm" {
                            let data = stream.decode_data(self)?;
                            let mut object_streams = ObjectStreams::try_new(stream.dict(), data)?;
                            for (objnum, obj) in object_streams.parse_objects()? {
                                let objinfo = self.cross_ref_table.lookup(&objnum).unwrap();
                                let gennum = 0;
                                println!("{objnum},{gennum}");
                                let reference = PdfReference::new(objnum, gennum);
                                self.indirect_objects.insert(reference, obj);
                            }
                        }
                    }
                }
                _ => {}
            }
            self.indirect_objects.insert(pdfref, obj);
        }
        Ok(())
    }

    pub fn get_root(&self) -> PdfResult<&PdfDict> {
        let root = self
            .cross_ref_table
            .trailer()
            .get("Root")
            .ok_or(PdfError::DocumentError(format!("Root not found")))?;

        let root = self.resolve(root)?;
        let root = root
            .as_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn resolve<'a>(&'a self, obj: &'a PdfObject) -> PdfResult<&'a PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => {
                if !self.indirect_objects.contains_key(reference) {
                    panic!("indirect object not exist:{:?}", reference);
                }
                let obj = self.indirect_objects.get(reference).unwrap();
                Ok(obj)
            }
            _ => Ok(obj),
        }
    }
}

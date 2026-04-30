use crate::objects::{
    object_id::ObjectId, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict,
    pdf_name::PdfName, pdf_number::PdfNumber, pdf_reference::PdfReference,
    pdf_stream::PdfStream, pdf_string::PdfString,
};

pub(crate) mod object_streams;
pub mod object_id;
pub mod pdf_array;
pub mod pdf_bool;
pub mod pdf_dict;
pub mod pdf_indirect;
pub mod pdf_name;
pub mod pdf_number;
pub mod pdf_reference;
pub mod pdf_stream;
pub mod pdf_string;

#[derive(Debug, PartialEq, Clone)]
pub enum PdfObject {
    PdfNull,
    PdfNumber(PdfNumber),
    PdfBool(PdfBool),
    PdfName(PdfName),
    PdfString(PdfString),
    PdfArray(PdfArray),
    PdfDict(PdfDict),
    PdfStream(PdfStream),
    PdfReference(PdfReference),
}

impl PdfObject {
    pub fn as_object_id(&self) -> Option<ObjectId> {
        self.as_reference().map(PdfReference::id)
    }

    pub fn as_u16(&self) -> Option<u16> {
        if let Some(num) = self.as_number() {
            Some(num.get_u16())
        } else {
            None
        }
    }
    pub fn as_i32(&self) -> Option<i32> {
        if let Some(num) = self.as_number() {
            Some(num.get_i32())
        } else {
            None
        }
    }

    pub fn as_u8(&self) -> Option<u8> {
        if let Some(num) = self.as_number() {
            Some(num.get_u8())
        } else {
            None
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        if let Some(num) = self.as_number() {
            Some(num.get_u32())
        } else {
            None
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        if let Some(num) = self.as_number() {
            Some(num.get_u64())
        } else {
            None
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        if let Some(num) = self.as_number() {
            Some(num.value())
        } else {
            None
        }
    }
    pub fn get_attr(&self, name: &str) -> Option<&PdfObject> {
        match self {
            PdfObject::PdfDict(dict) => dict.get(name),
            PdfObject::PdfStream(stream) => stream.dict().get(name),
            _ => None,
        }
    }
}

macro_rules! impl_pdf_cast {
    ($variant:ident, $inner:ty, $as_name:ident, $as_mut_name:ident, $into_name:ident) => {
        impl PdfObject {
            #[inline]
            pub fn $as_name(&self) -> Option<&$inner> {
                // If 'self' is a reference, 'v' becomes a reference automatically
                if let PdfObject::$variant(v) = self {
                    Some(v)
                } else {
                    None
                }
            }

            #[inline]
            pub fn $as_mut_name(&mut self) -> Option<&mut $inner> {
                // If 'self' is a mut reference, 'v' becomes a mut reference automatically
                if let PdfObject::$variant(v) = self {
                    Some(v)
                } else {
                    None
                }
            }

            #[inline]
            pub fn $into_name(self) -> Option<$inner> {
                // 'self' is owned here, so 'v' is moved out
                if let PdfObject::$variant(v) = self {
                    Some(v)
                } else {
                    None
                }
            }
        }
    };
}

impl_pdf_cast!(PdfNumber, PdfNumber, as_number, as_number_mut, into_number);
impl_pdf_cast!(PdfBool, PdfBool, as_bool, as_bool_mut, into_bool);
impl_pdf_cast!(PdfName, PdfName, as_name, as_name_mut, into_name);
impl_pdf_cast!(PdfString, PdfString, as_string, as_string_mut, into_string);
impl_pdf_cast!(PdfArray, PdfArray, as_array, as_array_mut, into_array);
impl_pdf_cast!(PdfDict, PdfDict, as_dict, as_dict_mut, into_dict);
impl_pdf_cast!(PdfStream, PdfStream, as_stream, as_stream_mut, into_stream);
impl_pdf_cast!(
    PdfReference,
    PdfReference,
    as_reference,
    as_reference_mut,
    into_reference
);

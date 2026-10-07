use crate::objects::{
    object_id::ObjectId, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict,
    pdf_name::PdfName, pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
    pdf_string::PdfString,
};

pub mod object_id;
pub(crate) mod object_streams;
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

/// Fallible payload/table cloning for an already-owned object. References are
/// copied as IDs, not resolved. Source allocations belong to their owner.
pub(crate) fn clone_object_with_budget(
    object: &PdfObject,
    budget: &crate::budget::OperationBudget,
    limits: crate::parser::options::Limits,
    depth: usize,
) -> crate::error::PdfResult<(PdfObject, crate::budget::MemoryLease)> {
    use crate::error::PdfError;
    if depth >= limits.max_depth {
        return Err(PdfError::ResourceLimit("object clone depth".into()));
    }
    match object {
        PdfObject::PdfNull | PdfObject::PdfBool(_) | PdfObject::PdfReference(_) => {
            Ok((object.clone(), budget.reserve(0)?))
        }
        PdfObject::PdfString(value) => {
            if value.bytes().len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("string clone bytes".into()));
            }
            let (copy, lease) = value.clone_with_budget(budget)?;
            Ok((PdfObject::PdfString(copy), lease))
        }
        PdfObject::PdfName(value) => {
            if value.bytes().len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("name clone bytes".into()));
            }
            let (copy, lease) = value.clone_with_budget(budget)?;
            Ok((PdfObject::PdfName(copy), lease))
        }
        PdfObject::PdfNumber(value) => {
            if value.lexeme().len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("number clone bytes".into()));
            }
            let (copy, lease) = value.clone_with_budget(budget)?;
            Ok((PdfObject::PdfNumber(copy), lease))
        }
        PdfObject::PdfArray(array) => {
            if array.len() > limits.max_objects {
                return Err(PdfError::ResourceLimit("array clone count".into()));
            }
            let mut lease = budget.reserve(
                array
                    .len()
                    .checked_mul(std::mem::size_of::<PdfObject>())
                    .ok_or_else(|| PdfError::ResourceLimit("array clone size".into()))?,
            )?;
            let mut items = Vec::new();
            items
                .try_reserve_exact(array.len())
                .map_err(|_| PdfError::ResourceLimit("array clone allocation".into()))?;
            lease.resize(
                items
                    .capacity()
                    .checked_mul(std::mem::size_of::<PdfObject>())
                    .ok_or_else(|| PdfError::ResourceLimit("array clone capacity".into()))?,
            )?;
            for item in array {
                let (copy, mut child) = clone_object_with_budget(item, budget, limits, depth + 1)?;
                items.push(copy);
                lease.absorb(&mut child)?;
            }
            Ok((PdfObject::PdfArray(PdfArray::from_items(items)), lease))
        }
        PdfObject::PdfDict(dict) => {
            let (copy, lease) = clone_dict_with_budget(dict, budget, limits, depth)?;
            Ok((PdfObject::PdfDict(copy), lease))
        }
        PdfObject::PdfStream(stream) => {
            let (copy, lease) = stream.clone_with_budget(budget, limits, depth)?;
            Ok((PdfObject::PdfStream(copy), lease))
        }
    }
}

pub(crate) fn clone_dict_with_budget(
    dict: &PdfDict,
    budget: &crate::budget::OperationBudget,
    limits: crate::parser::options::Limits,
    depth: usize,
) -> crate::error::PdfResult<(PdfDict, crate::budget::MemoryLease)> {
    use crate::error::PdfError;
    if depth >= limits.max_depth {
        return Err(PdfError::ResourceLimit("dictionary clone depth".into()));
    }
    if dict.into_iter().len() > limits.max_objects {
        return Err(PdfError::ResourceLimit("dictionary clone count".into()));
    }
    let mut builder = pdf_dict::BudgetedDictBuilder::new(budget, limits.max_objects)?;
    for (key, value) in dict {
        if key.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("dictionary clone key bytes".into()));
        }
        let (key, key_lease) = crate::budget::copy_string(key, budget)?;
        let (value, value_lease) = clone_object_with_budget(value, budget, limits, depth + 1)?;
        builder.insert(key, value, key_lease, value_lease)?;
    }
    builder.finish()
}

#[cfg(test)]
mod budget_clone_tests {
    use super::*;
    use crate::{
        budget::OperationBudget,
        io::stream_reader::StreamReader,
        parser::{options::Limits, syntax::SyntaxParser},
    };
    use std::io::Cursor;

    #[test]
    fn recursive_budget_clone_preserves_binary_names_lexemes_and_stream_state() {
        let source = b"<< /A [/#ff 12.00 (abc) true null 9 0 R] /B <00ff> >>";
        let mut parser = SyntaxParser::new(StreamReader::try_new(Cursor::new(source)).unwrap());
        let object = parser.get_object().unwrap();
        let pool = OperationBudget::new(32768);
        let (copy, lease) = clone_object_with_budget(&object, &pool, Limits::default(), 0).unwrap();
        assert_eq!(copy, object);
        let final_bytes = pool.live_bytes();
        let peak = pool.peak_bytes();
        assert!(peak > final_bytes);
        drop(copy);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(clone_object_with_budget(&object, &pool, Limits::default(), 0).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (copy, lease) = clone_object_with_budget(&object, &pool, Limits::default(), 0).unwrap();
        assert_eq!(pool.live_bytes(), final_bytes);
        drop(copy);
        drop(lease);
        let stream = PdfStream::from_decoded_data(PdfDict::default(), b"abc".to_vec()).unwrap();
        let (copy, lease) = stream
            .clone_with_budget(&pool, Limits::default(), 0)
            .unwrap();
        assert_eq!(copy, stream);
        assert_eq!(copy.data_state(), stream.data_state());
        drop(copy);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        for limits in [
            Limits {
                max_depth: 1,
                ..Default::default()
            },
            Limits {
                max_objects: 1,
                ..Default::default()
            },
            Limits {
                max_token_bytes: 1,
                ..Default::default()
            },
        ] {
            assert!(clone_object_with_budget(&object, &pool, limits, 0).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }
}

#[cfg(test)]
mod numeric_conversion_tests {
    use super::*;

    fn number(source: &str) -> PdfObject {
        PdfObject::PdfNumber(PdfNumber::new_from_lexeme(source.as_bytes()).unwrap())
    }

    #[test]
    fn integer_accessors_reject_fraction_and_overflow() {
        for source in ["1.5", "1.0"] {
            let object = number(source);
            assert!(object.as_u8().is_none());
            assert!(object.as_u16().is_none());
            assert!(object.as_u32().is_none());
            assert!(object.as_u64().is_none());
            assert!(object.as_i32().is_none());
        }
        assert_eq!(number("255").as_u8(), Some(255));
        assert!(number("256").as_u8().is_none());
        assert_eq!(number("65535").as_u16(), Some(65535));
        assert!(number("65536").as_u16().is_none());
        assert_eq!(number("4294967295").as_u32(), Some(u32::MAX));
        assert!(number("4294967296").as_u32().is_none());
        assert_eq!(number("18446744073709551615").as_u64(), Some(u64::MAX));
        assert!(number("18446744073709551616").as_u64().is_none());
        assert_eq!(number("-2147483648").as_i32(), Some(i32::MIN));
        assert_eq!(number("2147483647").as_i32(), Some(i32::MAX));
        assert!(number("2147483648").as_i32().is_none());
        assert!(number("-1").as_u64().is_none());
        assert!(PdfObject::PdfNull.as_u32().is_none());
    }
}

impl PdfObject {
    pub fn as_object_id(&self) -> Option<ObjectId> {
        self.as_reference().map(PdfReference::id)
    }

    pub fn as_u16(&self) -> Option<u16> {
        if let Some(num) = self.as_number() {
            num.as_u16_checked().ok()
        } else {
            None
        }
    }
    pub fn as_i32(&self) -> Option<i32> {
        if let Some(num) = self.as_number() {
            num.as_i32_checked().ok()
        } else {
            None
        }
    }

    pub fn as_u8(&self) -> Option<u8> {
        if let Some(num) = self.as_number() {
            num.as_u8_checked().ok()
        } else {
            None
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        if let Some(num) = self.as_number() {
            num.as_u32_checked().ok()
        } else {
            None
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        if let Some(num) = self.as_number() {
            num.as_u64_checked().ok()
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

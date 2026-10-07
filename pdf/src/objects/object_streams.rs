use std::io::Cursor;

use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{pdf_dict::PdfDict, PdfObject},
    parser::syntax::{ParseMode, SyntaxParser},
};

pub struct ObjectStreams<'a> {
    dict: &'a PdfDict,
    syntax: SyntaxParser<Cursor<Vec<u8>>>,
    data_length: u64,
}

pub(crate) struct BudgetedObjectStreams<'a> {
    stream: ObjectStreams<'a>,
    _data_lease: crate::budget::MemoryLease,
    budget: crate::budget::OperationBudget,
}

pub(crate) struct BudgetedStreamObjects {
    pub(crate) objects: Vec<(u32, PdfObject)>,
    pub(crate) lease: crate::budget::MemoryLease,
}

impl BudgetedObjectStreams<'_> {
    pub(crate) fn parse_objects(&mut self) -> PdfResult<BudgetedStreamObjects> {
        let (objects, lease) = self.stream.parse_objects_inner(Some(&self.budget))?;
        Ok(BudgetedStreamObjects {
            objects,
            lease: lease.unwrap(),
        })
    }
}

fn reserve_table<T>(
    count: usize,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(Vec<T>, Option<crate::budget::MemoryLease>)> {
    let Some(budget) = budget else {
        return Ok((Vec::new(), None));
    };
    let mut lease = budget.reserve(
        count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| PdfError::ResourceLimit("object stream table size".into()))?,
    )?;
    let mut table = Vec::new();
    table
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("object stream table allocation".into()))?;
    lease.resize(
        table
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| PdfError::ResourceLimit("object stream table capacity".into()))?,
    )?;
    Ok((table, Some(lease)))
}

fn object_stream_entry_is_forbidden_in_strict(obj: &PdfObject) -> bool {
    matches!(obj, PdfObject::PdfStream(_) | PdfObject::PdfReference(_))
}

impl<'a> ObjectStreams<'a> {
    pub(crate) fn try_new_with_budget(
        dict: &'a PdfDict,
        data: crate::budget::BudgetedBytes,
        options: crate::parser::options::ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedObjectStreams<'a>> {
        let mut retained = budget.reserve(0)?;
        let (data, mut lease) = data.into_reserved_parts();
        if let Err(error) = retained.absorb(&mut lease) {
            drop(data);
            return Err(error);
        }
        let stream = Self::try_new_with_options(dict, data, options)?;
        Ok(BudgetedObjectStreams {
            stream,
            _data_lease: retained,
            budget: budget.clone(),
        })
    }
    pub fn try_new(dict: &'a PdfDict, data: Vec<u8>) -> PdfResult<Self> {
        Self::try_new_with_mode(dict, data, ParseMode::Compatible)
    }

    pub fn try_new_with_mode(dict: &'a PdfDict, data: Vec<u8>, mode: ParseMode) -> PdfResult<Self> {
        Self::try_new_with_options(
            dict,
            data,
            crate::parser::options::ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }

    pub fn try_new_with_options(
        dict: &'a PdfDict,
        data: Vec<u8>,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<Self> {
        if data.len() > options.limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("object stream byte size".into()));
        }
        let data_length = data.len();
        let cursor = Cursor::new(data);
        let reader = StreamReader::try_new(cursor)?;
        let syntax = SyntaxParser::with_options(reader, options);
        Ok(Self {
            dict,
            syntax,
            data_length: data_length as u64,
        })
    }
    pub fn parse_objects(&mut self) -> PdfResult<Vec<(u32, PdfObject)>> {
        self.parse_objects_inner(None).map(|(objects, _)| objects)
    }

    fn parse_objects_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Vec<(u32, PdfObject)>, Option<crate::budget::MemoryLease>)> {
        let first = self
            .dict
            .get("First")
            .and_then(PdfObject::as_number)
            .ok_or(PdfError::ParserError(
                "object stream First must be an integer".to_string(),
            ))?
            .as_u64_checked()?;
        let n = self
            .dict
            .get("N")
            .and_then(PdfObject::as_number)
            .ok_or(PdfError::ParserError(
                "object stream N must be an integer".to_string(),
            ))?
            .as_u32_checked()?;
        if first > self.data_length {
            return Err(PdfError::ParserError(
                "object stream First points past stream data".to_string(),
            ));
        }
        if n as usize > self.syntax.limits().max_objects {
            return Err(PdfError::ResourceLimit("object stream N".into()));
        }
        let (_index_lease, mut objs) = {
            let (table, lease) = reserve_table(n as usize, budget)?;
            (lease, table)
        };
        let (_numbers_lease, mut sorted_numbers) = {
            let (table, lease) = reserve_table(n as usize, budget)?;
            (lease, table)
        };
        let mut object_numbers = std::collections::HashSet::new();
        let mut last_offset = 0;
        for i in 0..n {
            let (_number_lease, number) = if let Some(budget) = budget {
                let (number, lease) = self.syntax.get_number_with_budget(budget)?;
                (Some(lease), number)
            } else {
                (None, self.syntax.get_object()?)
            };
            let objnum = number
                .as_number()
                .ok_or(PdfError::ParserError(
                    "object stream object number must be an integer".to_string(),
                ))?
                .as_u32_checked()?;
            let (_offset_lease, offset_object) = if let Some(budget) = budget {
                let (offset, lease) = self.syntax.get_number_with_budget(budget)?;
                (Some(lease), offset)
            } else {
                (None, self.syntax.get_object()?)
            };
            let relative_offset = offset_object
                .as_number()
                .ok_or(PdfError::ParserError(
                    "object stream offset must be an integer".to_string(),
                ))?
                .as_u64_checked()?;
            let unique = if budget.is_some() {
                match sorted_numbers.binary_search(&objnum) {
                    Ok(_) => false,
                    Err(position) => {
                        sorted_numbers.insert(position, objnum);
                        true
                    }
                }
            } else {
                object_numbers.insert(objnum)
            };
            if objnum == 0 || !unique {
                return Err(PdfError::ParserError(
                    "object stream object numbers must be nonzero and unique".into(),
                ));
            }
            if self.syntax.position() > first {
                return Err(PdfError::ParserError(
                    "object stream header extends past First".into(),
                ));
            }
            let offset = relative_offset
                .checked_add(first)
                .ok_or_else(|| PdfError::ParserError("object stream offset overflow".into()))?;
            if offset > self.data_length {
                return Err(PdfError::ParserError(format!(
                    "object stream object offset {offset} points past stream data"
                )));
            }
            objs.push((objnum, offset));
            if i > 0 {
                let is_valid = if self.syntax.mode() == ParseMode::Strict {
                    offset > last_offset
                } else {
                    offset >= last_offset
                };
                if !is_valid {
                    return Err(PdfError::ParserError(
                        "object stream offsets must be in increasing order".to_string(),
                    ));
                }
            }
            last_offset = offset;
        }
        let mut retained = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut res = {
            let (table, lease) = reserve_table(n as usize, budget)?;
            if let Some(mut lease) = lease {
                retained.as_mut().unwrap().absorb(&mut lease)?;
            }
            table
        };
        for (index, (objnum, offset)) in objs.iter().enumerate() {
            self.syntax.set_pos(offset.to_owned())?;
            let end = objs
                .get(index + 1)
                .map(|(_, offset)| *offset)
                .unwrap_or(self.data_length);
            let (mut lease, obj) = if let Some(budget) = budget {
                let (obj, lease) = self.syntax.get_object_until_with_budget(end, budget)?;
                (Some(lease), obj)
            } else {
                (None, self.syntax.get_object_until(end)?)
            };
            if self.syntax.mode() == ParseMode::Strict
                && object_stream_entry_is_forbidden_in_strict(&obj)
            {
                return Err(PdfError::ParserError(format!(
                    "object stream entry {objnum} must not be a stream object in strict mode"
                )));
            }
            res.push((objnum.to_owned(), obj));
            if let Some(lease) = lease.as_mut() {
                retained.as_mut().unwrap().absorb(lease)?;
            }
        }
        Ok((res, retained))
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectStreams;
    use crate::{
        objects::{pdf_dict::PdfDict, pdf_number::PdfNumber, PdfObject},
        parser::syntax::ParseMode,
    };

    #[test]
    fn budgeted_object_stream_tables_payloads_and_boundaries() {
        use crate::{
            budget::{BudgetedBytes, OperationBudget},
            parser::options::ParseOptions,
        };
        let first = b"[(one) /Two] ";
        let header = format!("1 0 2 {} ", first.len());
        let mut data = header.as_bytes().to_vec();
        data.extend_from_slice(first);
        data.extend_from_slice(b"<< /Keep (two) >>");
        let mut dict = PdfDict::default();
        dict.insert(
            "First".into(),
            PdfObject::PdfNumber(PdfNumber::new(header.len() as f32)),
        );
        dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let options = ParseOptions {
                mode,
                ..Default::default()
            };
            let expected = ObjectStreams::try_new_with_options(&dict, data.clone(), options)
                .unwrap()
                .parse_objects()
                .unwrap();
            let pool = OperationBudget::new(1024 * 1024);
            let bytes = BudgetedBytes::copy_from_slice(&pool, &data).unwrap();
            let capacity = bytes.capacity();
            let mut stream =
                ObjectStreams::try_new_with_budget(&dict, bytes, options, &pool).unwrap();
            let parsed = stream.parse_objects().unwrap();
            assert_eq!(parsed.objects, expected);
            assert_eq!(pool.live_bytes(), capacity + parsed.lease.bytes());
            let peak = pool.peak_bytes();
            drop(stream);
            assert_eq!(pool.live_bytes(), parsed.lease.bytes());
            drop(parsed);
            assert_eq!(pool.live_bytes(), 0);
            let small = OperationBudget::new(peak - 1);
            let bytes = BudgetedBytes::copy_from_slice(&small, &data).unwrap();
            let mut stream =
                ObjectStreams::try_new_with_budget(&dict, bytes, options, &small).unwrap();
            assert!(matches!(
                stream.parse_objects(),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            drop(stream);
            assert_eq!(small.live_bytes(), 0);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let bytes = BudgetedBytes::copy_from_slice(&pool, &data).unwrap();
                let mut stream =
                    ObjectStreams::try_new_with_budget(&dict, bytes, options, &pool).unwrap();
                let _parsed = stream.parse_objects().unwrap();
                panic!("object stream owner unwind");
            }));
            assert!(caught.is_err());
            assert_eq!(pool.live_bytes(), 0);
            let mut invalid = PdfDict::default();
            invalid.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(8.0)));
            invalid.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
            for data in [
                b"1 0 1 5 true false".as_slice(),
                b"1 0 2 3 (a )".as_slice(),
                b"1 0 2 5 true false extra".as_slice(),
            ] {
                assert!(
                    ObjectStreams::try_new_with_options(&invalid, data.to_vec(), options)
                        .unwrap()
                        .parse_objects()
                        .is_err()
                );
                let bytes = BudgetedBytes::copy_from_slice(&pool, data).unwrap();
                let mut stream =
                    ObjectStreams::try_new_with_budget(&invalid, bytes, options, &pool).unwrap();
                assert!(stream.parse_objects().is_err());
                drop(stream);
                assert_eq!(pool.live_bytes(), 0);
            }
        }
        let pool = OperationBudget::new(65536);
        let foreign = OperationBudget::new(65536);
        let bytes = BudgetedBytes::copy_from_slice(&foreign, &data).unwrap();
        assert!(
            ObjectStreams::try_new_with_budget(&dict, bytes, Default::default(), &pool).is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(foreign.live_bytes(), 0);
    }

    #[test]
    fn malformed_object_stream_headers_return_errors() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let mut dict = PdfDict::default();
            dict.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(8.0)));
            dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
            let mut valid =
                ObjectStreams::try_new_with_mode(&dict, b"1 0 2 5 true false".to_vec(), mode)
                    .unwrap();
            let objects = valid.parse_objects().unwrap();
            assert_eq!(
                objects.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                vec![1, 2]
            );
            for (first, n, data) in [
                (25.0, 1.0, "1 18446744073709551615    true"),
                (8.0, 2.0, "1 0 1 5 true false"),
                (4.0, 1.0, "0 0 true"),
                (2.0, 1.0, "1 0 true"),
            ] {
                let mut dict = PdfDict::default();
                dict.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(first)));
                dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(n)));
                let mut stream =
                    ObjectStreams::try_new_with_mode(&dict, data.as_bytes().to_vec(), mode)
                        .unwrap();
                assert!(stream.parse_objects().is_err(), "{data:?} {mode:?}");
            }
        }
    }

    #[test]
    fn test_strict_object_stream_rejects_stream_object() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(4.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        let data = b"1 0 << /Length 0 >>\nstream\n\nendstream".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }

    #[test]
    fn object_stream_entries_cannot_consume_next_entry() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            for data in ["1 0 2 3 (a )", "1 0 2 3 [1 ]", "1 0 2 3 <ab>"] {
                let mut dict = PdfDict::default();
                dict.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(8.0)));
                dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
                let mut stream =
                    ObjectStreams::try_new_with_mode(&dict, data.as_bytes().to_vec(), mode)
                        .unwrap();
                assert!(stream.parse_objects().is_err(), "{data:?}");
            }
        }
    }

    #[test]
    fn object_stream_entry_rejects_extra_object_but_allows_comments() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let mut dict = PdfDict::default();
            dict.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(4.0)));
            dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
            for (data, valid) in [
                ("1 0 true false", false),
                ("1 0 42 43", false),
                ("1 0 ", false),
                ("1 0 true % comment", true),
                ("1 0 42 % comment\r\n \t", true),
            ] {
                let mut stream =
                    ObjectStreams::try_new_with_mode(&dict, data.as_bytes().to_vec(), mode)
                        .unwrap();
                assert_eq!(stream.parse_objects().is_ok(), valid, "{data:?}");
            }
        }
    }

    #[test]
    fn test_strict_object_stream_rejects_reference_object() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(4.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        let data = b"1 0 3 0 R".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }

    #[test]
    fn test_strict_object_stream_requires_strictly_increasing_offsets() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(8.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        let data = b"1 0 2 0 truefalse".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }
}

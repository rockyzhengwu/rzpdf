use std::collections::HashMap;

use super::reader::TrueTypeReader;
use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone)]
pub struct TableRecord {
    tag: String,
    check_sum: u32,
    offset: u32,
    length: u32,
}

impl TableRecord {
    pub fn try_new(reader: &mut TrueTypeReader) -> PdfResult<Self> {
        let tag = reader.read_tag()?;
        let check_sum = reader.read_be_u32()?;
        let offset = reader.read_be_u32()?;
        let length = reader.read_be_u32()?;
        Ok(Self {
            tag,
            check_sum,
            offset,
            length,
        })
    }
    pub fn offset(&self) -> u32 {
        self.offset
    }
    pub fn length(&self) -> u32 {
        self.length
    }
}

#[derive(Debug, Clone)]
pub struct Sfnt {
    tables: HashMap<String, TableRecord>,
}
impl Sfnt {
    pub fn try_new(reader: &mut TrueTypeReader) -> PdfResult<Self> {
        Self::try_new_with_limits(reader, crate::parser::options::Limits::default())
    }

    pub fn try_new_with_limits(
        reader: &mut TrueTypeReader,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Self> {
        crate::filter::check_output_size(
            0,
            reader.length(),
            limits.max_decoded_bytes,
            "sfnt font bytes",
        )?;
        let _version = reader.read_be_u32()?;
        let num_tables = reader.read_be_u16()?;
        crate::filter::check_output_size(
            0,
            usize::from(num_tables),
            limits.max_objects,
            "sfnt table records",
        )?;
        let search_range = reader.read_be_u16()?;
        let _entry_selector = reader.read_be_u16()?;
        let range_shift = reader.read_be_u16()?;
        if num_tables == 0 {
            return Ok(Sfnt {
                tables: HashMap::new(),
            });
        }
        let directory_bytes = usize::from(num_tables) * 16;
        if directory_bytes > reader.remaining()
            || usize::from(search_range) > directory_bytes
            || usize::from(range_shift) != directory_bytes - usize::from(search_range)
        {
            return Err(PdfError::FontError("Invalid sfnt table directory".into()));
        }
        let mut tables = HashMap::new();
        tables
            .try_reserve(usize::from(num_tables))
            .map_err(|_| PdfError::ResourceLimit("sfnt table directory allocation".into()))?;
        for _ in 0..num_tables {
            let record = TableRecord::try_new(reader)?;
            let end = u64::from(record.offset) + u64::from(record.length);
            if end > reader.length() as u64 {
                return Err(PdfError::FontError("sfnt table exceeds font data".into()));
            }
            if tables.contains_key(&record.tag) {
                return Err(PdfError::FontError("Duplicate sfnt table tag".into()));
            }
            tables.insert(record.tag.clone(), record);
        }
        Ok(Sfnt { tables })
    }
    pub fn get_table(&self, name: &str) -> Option<&TableRecord> {
        self.tables.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sfnt_limits_are_checked_before_table_allocation() {
        use crate::parser::options::Limits;
        let mut font = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        font.extend_from_slice(b"test");
        font.extend_from_slice(&0u32.to_be_bytes());
        font.extend_from_slice(&28u32.to_be_bytes());
        font.extend_from_slice(&0u32.to_be_bytes());
        let exact = Limits {
            max_objects: 1,
            max_decoded_bytes: font.len(),
            ..Limits::default()
        };
        assert!(Sfnt::try_new_with_limits(&mut TrueTypeReader::new(&font), exact).is_ok());
        for limits in [
            Limits {
                max_objects: 0,
                ..exact
            },
            Limits {
                max_decoded_bytes: font.len() - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                Sfnt::try_new_with_limits(&mut TrueTypeReader::new(&font), limits),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        let header = [0, 1, 0, 0, 0xff, 0xff];
        assert!(matches!(
            Sfnt::try_new_with_limits(&mut TrueTypeReader::new(&header), exact),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(matches!(
            crate::font::truetype::true_type_program::TrueTypeProgram::try_new_with_limits(
                &font,
                Limits {
                    max_objects: 0,
                    ..exact
                }
            ),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn sfnt_directory_checks_arithmetic_bounds_and_duplicates() {
        let mut valid = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        valid.extend_from_slice(b"test");
        valid.extend_from_slice(&0_u32.to_be_bytes());
        valid.extend_from_slice(&28_u32.to_be_bytes());
        valid.extend_from_slice(&0_u32.to_be_bytes());
        assert!(Sfnt::try_new(&mut TrueTypeReader::new(&valid)).is_ok());
        for end in 0..valid.len() {
            assert!(Sfnt::try_new(&mut TrueTypeReader::new(&valid[..end])).is_err());
        }
        for (index, value) in [(7, 17), (11, 1), (23, 255), (27, 1)] {
            let mut bad = valid.clone();
            bad[index] = value;
            assert!(Sfnt::try_new(&mut TrueTypeReader::new(&bad)).is_err());
        }
        let mut huge = valid.clone();
        huge[4..6].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(Sfnt::try_new(&mut TrueTypeReader::new(&huge)).is_err());
        let mut duplicate = valid.clone();
        duplicate[5] = 2;
        duplicate[7] = 32;
        duplicate.extend_from_slice(&valid[12..]);
        assert!(Sfnt::try_new(&mut TrueTypeReader::new(&duplicate)).is_err());
    }
}

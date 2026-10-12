use std::collections::HashMap;

use super::reader::TrueTypeReader;
use crate::error::{PdfError, PdfResult};

/// SFNT unsigned big-endian word sum, with virtual zero padding. For head's
/// directory checksum, its checkSumAdjustment bytes must already be zero.
pub(crate) fn table_checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[derive(Debug, Clone)]
pub struct TableRecord {
    tag: String,
    check_sum: u32,
    offset: u32,
    length: u32,
}

impl TableRecord {
    pub(crate) fn tag(&self) -> &str {
        &self.tag
    }

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

    /// Borrowed, unordered records for explicit subset table policy. A writer
    /// must sort its output directory by tag; hash iteration is not file order.
    pub(crate) fn table_records(&self) -> impl ExactSizeIterator<Item = &TableRecord> {
        self.tables.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_handles_virtual_padding_wraparound_and_font_adjustment() {
        assert_eq!(table_checksum(&[]), 0);
        assert_eq!(table_checksum(&[1]), 0x01000000);
        assert_eq!(table_checksum(&[1, 2]), 0x01020000);
        assert_eq!(table_checksum(&[1, 2, 3]), 0x01020300);
        assert_eq!(table_checksum(&[1, 2, 3, 4, 5]), 0x06020304);
        assert_eq!(table_checksum(&[255, 255, 255, 255, 0, 0, 0, 1]), 0);
        let mut font = [0u8; 16];
        font[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        let adjustment = 0xB1B0AFBAu32.wrapping_sub(table_checksum(&font));
        font[8..12].copy_from_slice(&adjustment.to_be_bytes());
        assert_eq!(table_checksum(&font), 0xB1B0AFBA);
    }

    #[test]
    fn record_iteration_includes_unknown_tables_without_copying() {
        let mut font = vec![0, 1, 0, 0, 0, 2, 0, 32, 0, 1, 0, 0];
        for tag in [b"head", b"ZZZZ"] {
            font.extend_from_slice(tag);
            font.extend_from_slice(&0u32.to_be_bytes());
            font.extend_from_slice(&44u32.to_be_bytes());
            font.extend_from_slice(&0u32.to_be_bytes());
        }
        let sfnt = Sfnt::try_new(&mut TrueTypeReader::new(&font)).unwrap();
        assert_eq!(sfnt.table_records().len(), 2);
        let mut tags: Vec<_> = sfnt.table_records().map(TableRecord::tag).collect();
        tags.sort_unstable();
        assert_eq!(tags, ["ZZZZ", "head"]);
        for record in sfnt.table_records() {
            assert!(std::ptr::eq(record, sfnt.get_table(record.tag()).unwrap()));
        }
    }

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

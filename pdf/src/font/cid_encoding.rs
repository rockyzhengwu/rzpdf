//! Immutable two-byte Identity-H repertoire for a future embedded CIDFontType2.
use crate::{
    error::{PdfError, PdfResult},
    font::truetype::true_type_program::TrueTypeProgram,
    parser::options::Limits,
};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub struct UnicodeCidEncoding {
    entries: Vec<(char, u16, f64)>,
    limits: Limits,
}

impl UnicodeCidEncoding {
    /// Assign one CID per Unicode scalar, even when scalars share a glyph.
    /// CID zero is reserved for .notdef. No shaping or fallback is performed.
    pub fn from_text(font: &TrueTypeProgram, repertoire: &str, limits: Limits) -> PdfResult<Self> {
        let metrics = font
            .horizontal_metrics()
            .ok_or_else(|| PdfError::FontError("CID widths require horizontal metrics".into()))?;
        Self::build(repertoire, limits, |character| {
            let gid = font.unicode_glyph_id(character)?.ok_or_else(|| {
                PdfError::FontError(format!("Missing glyph for U+{:04X}", character as u32))
            })?;
            Ok((gid, metrics.advance_width_1000(gid)?))
        })
    }

    fn build(
        repertoire: &str,
        limits: Limits,
        mut lookup: impl FnMut(char) -> PdfResult<(u16, f64)>,
    ) -> PdfResult<Self> {
        let count = repertoire.chars().count();
        if count > limits.max_text_chars
            || count
                .checked_mul(std::mem::size_of::<char>())
                .is_none_or(|size| size > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("CID repertoire size".into()));
        }
        let mut characters = Vec::new();
        characters
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("CID repertoire allocation".into()))?;
        characters.extend(repertoire.chars());
        characters.sort_unstable();
        characters.dedup();
        if characters.len() > usize::from(u16::MAX)
            || characters.len() > limits.max_objects
            || characters
                .len()
                .checked_mul(std::mem::size_of::<(char, u16, f64)>())
                .is_none_or(|size| size > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("CID entry count or bytes".into()));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(characters.len())
            .map_err(|_| PdfError::ResourceLimit("CID entry allocation".into()))?;
        for character in characters {
            let (gid, width) = lookup(character)?;
            if gid == 0 || !width.is_finite() || width < 0.0 {
                return Err(PdfError::FontError("Invalid CID glyph or width".into()));
            }
            entries.push((character, gid, width));
        }
        Ok(Self { entries, limits })
    }

    /// Text must use the fixed repertoire established before font registration.
    pub fn encode_text(&self, text: &str) -> PdfResult<Vec<u8>> {
        let count = text.chars().count();
        let size = count
            .checked_mul(2)
            .ok_or_else(|| PdfError::ResourceLimit("CID text size overflow".into()))?;
        if count > self.limits.max_text_chars || size > self.limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("CID text size".into()));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("CID text allocation".into()))?;
        for character in text.chars() {
            let index = self
                .entries
                .binary_search_by_key(&character, |entry| entry.0)
                .map_err(|_| {
                    PdfError::FontError(format!(
                        "Character U+{:04X} is outside CID repertoire",
                        character as u32
                    ))
                })?;
            output.extend_from_slice(&((index + 1) as u16).to_be_bytes());
        }
        Ok(output)
    }

    pub fn widths(&self) -> impl Iterator<Item = (u16, f64)> + '_ {
        self.glyph_mappings().map(|(cid, _, _, width)| (cid, width))
    }

    /// Keep the encoded payload and its actual capacity under one budget owner.
    /// The immutable repertoire itself remains owned by the registered font.
    pub(crate) fn encode_text_with_budget(
        &self,
        text: &str,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let count = crate::font::pdf_font::encoding_character_count(text, self.limits)?;
        let length = count
            .checked_mul(2)
            .filter(|&value| value <= self.limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("CID text size".into()))?;
        let mut lease = budget.reserve(length)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("CID text allocation".into()))?;
        lease.resize(output.capacity())?;
        for character in text.chars() {
            let index = self
                .entries
                .binary_search_by_key(&character, |entry| entry.0)
                .map_err(|_| {
                    PdfError::FontError(format!(
                        "Character U+{:04X} is outside CID repertoire",
                        character as u32
                    ))
                })?;
            output.extend_from_slice(&((index + 1) as u16).to_be_bytes());
        }
        crate::budget::BudgetedBytes::from_reserved_vec(output, lease)
    }

    /// Original-font GIDs, not subset-renumbered IDs. Borrowed traversal does
    /// not allocate or merge distinct Unicode scalars sharing the same glyph.
    /// CID zero/.notdef is implicit and must be added by subset planning.
    pub(crate) fn glyph_mappings(
        &self,
    ) -> impl ExactSizeIterator<Item = (u16, char, u16, f64)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| ((index + 1) as u16, entry.0, entry.1, entry.2))
    }

    pub fn cid_to_gid_map(&self) -> PdfResult<Vec<u8>> {
        let size = (self.entries.len() + 1) * 2;
        if size > self.limits.max_decoded_bytes || size > self.limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("CIDToGIDMap bytes".into()));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("CIDToGIDMap allocation".into()))?;
        output.extend_from_slice(&[0, 0]);
        for (_, _, gid, _) in self.glyph_mappings() {
            output.extend_from_slice(&gid.to_be_bytes());
        }
        Ok(output)
    }

    /// Standalone Unicode-platform (0/4) format-12 cmap for the fixed scalar
    /// repertoire. Original GIDs stay unchanged; no variation/shaping mapping.
    pub(crate) fn subset_cmap(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let length = self
            .entries
            .len()
            .checked_mul(12)
            .and_then(|bytes| bytes.checked_add(28))
            .filter(|&bytes| {
                bytes <= self.limits.max_decoded_bytes
                    && bytes <= self.limits.max_raw_stream_bytes
                    && bytes <= u32::MAX as usize
            })
            .ok_or_else(|| PdfError::ResourceLimit("Subset cmap bytes".into()))?;
        let mut lease = budget.reserve(length)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("Subset cmap allocation".into()))?;
        lease.resize(output.capacity())?;
        // One Unicode full-repertoire record, pointing at the format-12 table.
        output.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, 12]);
        output.extend_from_slice(&[0, 12, 0, 0]);
        output.extend_from_slice(&((length - 12) as u32).to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());
        output.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for (_, character, gid, _) in self.glyph_mappings() {
            // Singleton groups also preserve many-to-one glyph mappings.
            output.extend_from_slice(&(character as u32).to_be_bytes());
            output.extend_from_slice(&(character as u32).to_be_bytes());
            output.extend_from_slice(&u32::from(gid).to_be_bytes());
        }
        crate::budget::BudgetedBytes::from_reserved_vec(output, lease)
    }

    /// UTF-16BE destinations, including surrogate pairs; no BOM or UTF-8 bytes.
    pub fn to_unicode_cmap(&self) -> PdfResult<Vec<u8>> {
        let mut output = String::new();
        let limit = self
            .limits
            .max_decoded_bytes
            .min(self.limits.max_raw_stream_bytes);
        let mut append = |part: &str| -> PdfResult<()> {
            if output
                .len()
                .checked_add(part.len())
                .is_none_or(|size| size > limit)
            {
                return Err(PdfError::ResourceLimit("ToUnicode bytes".into()));
            }
            output
                .try_reserve(part.len())
                .map_err(|_| PdfError::ResourceLimit("ToUnicode allocation".into()))?;
            output.push_str(part);
            Ok(())
        };
        append(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Rzpdf-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        )?;
        for (chunk_index, chunk) in self.entries.chunks(100).enumerate() {
            append(&format!("{} beginbfchar\n", chunk.len()))?;
            for (index, entry) in chunk.iter().enumerate() {
                let mut line = format!("<{:04X}> <", chunk_index * 100 + index + 1);
                for unit in entry.0.encode_utf16(&mut [0; 2]) {
                    write!(line, "{unit:04X}").unwrap();
                }
                line.push_str(">\n");
                append(&line)?;
            }
            append("endbfchar\n")?;
        }
        append("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n")?;
        Ok(output.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgeted_encoding_owns_capacity_and_releases_on_failure_and_retry() {
        let encoding =
            UnicodeCidEncoding::build("A中𠀀", Limits::default(), |_| Ok((7, 500.0))).unwrap();
        let pool = crate::budget::OperationBudget::new(4096);
        let neighbor = pool.reserve(7).unwrap();
        let output = encoding.encode_text_with_budget("中A𠀀", &pool).unwrap();
        assert_eq!(output.as_slice(), encoding.encode_text("中A𠀀").unwrap());
        assert!(pool.live_bytes() >= 13);
        drop(output);
        assert_eq!(pool.live_bytes(), 7);
        assert!(encoding.encode_text_with_budget("中B", &pool).is_err());
        assert_eq!(pool.live_bytes(), 7);
        let short = crate::budget::OperationBudget::new(5);
        assert!(encoding.encode_text_with_budget("中A𠀀", &short).is_err());
        assert_eq!(short.live_bytes(), 0);
        drop(encoding.encode_text_with_budget("中A𠀀", &pool).unwrap());
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn subset_cmap_keeps_scalar_gids_and_rejects_budget_without_leaks() {
        let encoding =
            UnicodeCidEncoding::build("中𠀀A中", Limits::default(), |_| Ok((7, 500.0))).unwrap();
        let pool = crate::budget::OperationBudget::new(4096);
        let bytes = encoding.subset_cmap(&pool).unwrap();
        let cmap = crate::font::truetype::tables::cmap::TCmap::try_new(
            0,
            4,
            12,
            &mut crate::font::truetype::reader::TrueTypeReader::new(bytes.as_slice()),
        )
        .unwrap();
        for scalar in ['A', '中', '𠀀'] {
            assert_eq!(cmap.get_gid_u32(scalar as u32), 7);
        }
        assert_eq!(cmap.get_gid_u32('B' as u32), 0);
        assert_eq!(bytes.as_slice().len(), 64);
        drop(bytes);
        assert_eq!(pool.live_bytes(), 0);
        let short = crate::budget::OperationBudget::new(63);
        assert!(encoding.subset_cmap(&short).is_err());
        assert_eq!(short.live_bytes(), 0);
    }

    #[test]
    fn distinct_scalars_sharing_glyph_keep_distinct_cids_and_unicode() {
        let encoding =
            UnicodeCidEncoding::build("中𠀀中A", Limits::default(), |_| Ok((7, 500.5))).unwrap();
        assert_eq!(
            encoding.encode_text("中A𠀀中").unwrap(),
            [0, 2, 0, 1, 0, 3, 0, 2]
        );
        assert_eq!(encoding.cid_to_gid_map().unwrap(), [0, 0, 0, 7, 0, 7, 0, 7]);
        let mappings = encoding.glyph_mappings();
        assert_eq!(mappings.len(), 3);
        assert_eq!(
            mappings.collect::<Vec<_>>(),
            [(1, 'A', 7, 500.5), (2, '中', 7, 500.5), (3, '𠀀', 7, 500.5)]
        );
        assert_eq!(
            encoding.widths().collect::<Vec<_>>(),
            [(1, 500.5), (2, 500.5), (3, 500.5)]
        );
        let cmap = String::from_utf8(encoding.to_unicode_cmap().unwrap()).unwrap();
        assert!(cmap.contains("<0002> <4E2D>"));
        assert!(cmap.contains("<0003> <D840DC00>"));
        assert!(encoding.encode_text("B").is_err());
    }

    #[test]
    fn cmap_chunks_and_limits_are_checked() {
        let text: String = (0x100..0x165)
            .map(|code| char::from_u32(code).unwrap())
            .collect();
        let encoding =
            UnicodeCidEncoding::build(&text, Limits::default(), |_| Ok((1, 0.0))).unwrap();
        let cmap = String::from_utf8(encoding.to_unicode_cmap().unwrap()).unwrap();
        assert!(cmap.contains("100 beginbfchar"));
        assert!(cmap.contains("1 beginbfchar\n<0065> <0164>"));
        let limits = Limits {
            max_objects: 1,
            ..Limits::default()
        };
        assert!(UnicodeCidEncoding::build("AB", limits, |_| Ok((1, 0.0))).is_err());
        assert!(UnicodeCidEncoding::build("A", Limits::default(), |_| Ok((0, 0.0))).is_err());
        assert!(UnicodeCidEncoding::build("A", Limits::default(), |_| Ok((1, f64::NAN))).is_err());
        let small = UnicodeCidEncoding::build(
            "A",
            Limits {
                max_raw_stream_bytes: 1,
                ..Limits::default()
            },
            |_| Ok((1, 0.0)),
        )
        .unwrap();
        assert!(small.cid_to_gid_map().is_err());
        assert!(small.to_unicode_cmap().is_err());
    }
}

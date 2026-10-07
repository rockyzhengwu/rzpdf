use crate::error::{PdfError, PdfResult};
use crate::font::truetype::{reader::TrueTypeReader, sfnt::Sfnt, tables::cmap::TCmap};

#[derive(Debug, Clone)]
pub struct TrueTypeProgram {
    post_metrics: Option<crate::font::truetype::tables::post::PostMetrics>,
    horizontal_metrics: Option<crate::font::truetype::tables::horizontal::HorizontalMetrics>,
    embedding_rights: Option<crate::font::truetype::tables::os2::FontEmbeddingRights>,
    num_glyphs: Option<u16>,
    cmaps: Vec<TCmap>,
    current: Option<usize>,
}

impl TrueTypeProgram {
    /// Extract a simple TrueType glyph's unhinted quadratic outline in em units.
    /// Empty loca ranges yield no commands. Composite/CFF outlines reject; this
    /// does not select a PDF character's GID or apply variable-font deltas.
    pub fn checked_simple_glyph_outline(
        data: &[u8],
        gid: u16,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        Self::checked_outline(data, gid, limits, false)
    }

    /// Unhinted simple/composite default-instance outline in em units.
    /// Phantom-point alignment, hinting and variable deltas are not supported.
    pub fn checked_glyph_outline(
        data: &[u8],
        gid: u16,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        Self::checked_outline(data, gid, limits, true)
    }

    fn checked_outline(
        data: &[u8],
        gid: u16,
        limits: crate::parser::options::Limits,
        expand_composites: bool,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        use crate::font::truetype::tables::glyf::GlyphOutlineCommand;
        let locations = Self::checked_glyph_locations(data, limits)?;
        let mut reader = TrueTypeReader::new(data);
        let sfnt = Sfnt::try_new_with_limits(&mut reader, limits)?;
        let head = sfnt
            .get_table("head")
            .ok_or_else(|| PdfError::FontError("Missing head table".into()))?;
        let start = (head.offset() as usize)
            .checked_add(18)
            .ok_or_else(|| PdfError::FontError("head offset overflow".into()))?;
        if head.length() < 20 {
            return Err(PdfError::FontError("Truncated head unitsPerEm".into()));
        }
        let bytes = data
            .get(start..start + 2)
            .ok_or_else(|| PdfError::FontError("Truncated unitsPerEm".into()))?;
        let units = u16::from_be_bytes([bytes[0], bytes[1]]);
        if !(16..=16384).contains(&units) {
            return Err(PdfError::FontError("Invalid head unitsPerEm".into()));
        }
        let mut commands = if expand_composites {
            locations
                .expanded_glyph(gid, &limits)?
                .outline_commands(&limits)?
        } else {
            let Some(glyph) = locations.simple_glyph(gid, &limits)? else {
                return Ok(Vec::new());
            };
            glyph.outline_commands(&limits)?
        };
        let normalize = |point: &mut [f64; 2]| {
            point[0] /= f64::from(units);
            point[1] /= f64::from(units);
        };
        for command in &mut commands {
            match command {
                GlyphOutlineCommand::Move(point) | GlyphOutlineCommand::Line(point) => {
                    normalize(point)
                }
                GlyphOutlineCommand::Quadratic { control, end } => {
                    normalize(control);
                    normalize(end);
                }
                GlyphOutlineCommand::Close => {}
            }
        }
        Ok(commands)
    }

    /// Inspect TrueType glyph byte ranges without copying the outline table.
    /// This does not validate simple/composite glyph bodies or hinting programs.
    pub fn checked_glyph_locations(
        data: &[u8],
        limits: crate::parser::options::Limits,
    ) -> PdfResult<crate::font::truetype::tables::loca::GlyphLocations<'_>> {
        let program = Self::try_new_with_limits(data, limits)?;
        if data.get(..4) == Some(b"OTTO") {
            return Err(PdfError::UnsupportedFeature("CFF glyph locations".into()));
        }
        let mut reader = TrueTypeReader::new(data);
        let sfnt = Sfnt::try_new_with_limits(&mut reader, limits)?;
        let table = |name: &str| -> PdfResult<&[u8]> {
            let record = sfnt
                .get_table(name)
                .ok_or_else(|| PdfError::FontError(format!("Missing {name} table")))?;
            let start = record.offset() as usize;
            let end = start
                .checked_add(record.length() as usize)
                .ok_or_else(|| PdfError::FontError("Glyph table range overflow".into()))?;
            data.get(start..end)
                .ok_or_else(|| PdfError::FontError("Glyph table exceeds font data".into()))
        };
        let head = table("head")?;
        let format = head
            .get(50..52)
            .ok_or_else(|| PdfError::FontError("Truncated head indexToLocFormat".into()))?;
        crate::font::truetype::tables::loca::GlyphLocations::try_new(
            table("loca")?,
            table("glyf")?,
            program
                .num_glyphs
                .ok_or_else(|| PdfError::FontError("Glyph locations require maxp".into()))?,
            i16::from_be_bytes([format[0], format[1]]),
        )
    }

    pub fn try_new(data: &[u8]) -> PdfResult<Self> {
        Self::try_new_with_limits(data, crate::parser::options::Limits::default())
    }

    pub fn try_new_with_limits(
        data: &[u8],
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Self> {
        crate::filter::check_output_size(
            0,
            data.len(),
            limits.max_decoded_bytes,
            "TrueType font bytes",
        )?;
        let signature = data
            .get(..4)
            .ok_or_else(|| PdfError::FontError("Truncated SFNT signature".into()))?;
        if signature != [0, 1, 0, 0] && signature != b"OTTO" && signature != b"true" {
            return Err(PdfError::UnsupportedFeature(
                "Font is not a supported SFNT program".into(),
            ));
        }
        let mut reader = TrueTypeReader::new(data);
        let sfnt = Sfnt::try_new_with_limits(&mut reader, limits)?;
        let post_metrics = sfnt
            .get_table("post")
            .map(|record| {
                let start = record.offset() as usize;
                let end = start
                    .checked_add(record.length() as usize)
                    .ok_or_else(|| PdfError::FontError("post range overflow".into()))?;
                let table = data
                    .get(start..end)
                    .ok_or_else(|| PdfError::FontError("post exceeds font data".into()))?;
                crate::font::truetype::tables::post::PostMetrics::from_header(table)
            })
            .transpose()?;
        let embedding_rights = sfnt
            .get_table("OS/2")
            .map(|record| {
                let start = record.offset() as usize;
                let end = start
                    .checked_add(record.length() as usize)
                    .ok_or_else(|| PdfError::FontError("OS/2 range overflow".into()))?;
                let table = data
                    .get(start..end)
                    .ok_or_else(|| PdfError::FontError("OS/2 exceeds font data".into()))?;
                crate::font::truetype::tables::os2::FontEmbeddingRights::from_os2_prefix(table)
            })
            .transpose()?;
        let num_glyphs = if let Some(record) = sfnt.get_table("maxp") {
            let start = record.offset() as usize;
            let end = start
                .checked_add(record.length() as usize)
                .ok_or_else(|| PdfError::FontError("maxp range overflow".into()))?;
            let table = data
                .get(start..end)
                .ok_or_else(|| PdfError::FontError("maxp exceeds font data".into()))?;
            let mut reader = TrueTypeReader::new(table);
            let version = reader.read_be_u32()?;
            let required = match version {
                0x00005000 => 6,
                0x00010000 => 32,
                _ => return Err(PdfError::UnsupportedFeature("maxp version".into())),
            };
            if table.len() < required {
                return Err(PdfError::FontError("Truncated maxp table".into()));
            }
            let count = reader.read_be_u16()?;
            if count == 0 {
                return Err(PdfError::FontError("maxp has no glyphs".into()));
            }
            Some(count)
        } else {
            None
        };

        let horizontal_metrics = if let (Some(head), Some(hhea), Some(hmtx)) = (
            sfnt.get_table("head"),
            sfnt.get_table("hhea"),
            sfnt.get_table("hmtx"),
        ) {
            let table = |record: &crate::font::truetype::sfnt::TableRecord| -> PdfResult<&[u8]> {
                let start = record.offset() as usize;
                let end = start
                    .checked_add(record.length() as usize)
                    .ok_or_else(|| PdfError::FontError("metrics table range overflow".into()))?;
                data.get(start..end)
                    .ok_or_else(|| PdfError::FontError("metrics table exceeds font data".into()))
            };
            Some(
                crate::font::truetype::tables::horizontal::HorizontalMetrics::try_new(
                    table(head)?,
                    table(hhea)?,
                    table(hmtx)?,
                    num_glyphs.ok_or_else(|| {
                        PdfError::FontError("horizontal metrics require maxp".into())
                    })?,
                )?,
            )
        } else {
            None
        };
        if let Some(cmap_record) = sfnt.get_table("cmap") {
            let start = cmap_record.offset() as usize;
            let end = start
                .checked_add(cmap_record.length() as usize)
                .ok_or_else(|| PdfError::FontError("cmap table range overflow".into()))?;
            let table = data
                .get(start..end)
                .ok_or_else(|| PdfError::FontError("cmap table exceeds font data".into()))?;
            let mut reader = TrueTypeReader::new(table);
            let version = reader.read_be_u16()?;
            if version != 0 {
                return Err(PdfError::FontError("Invalid cmap version".into()));
            }
            let num_tables = reader.read_be_u16()?;
            crate::filter::check_output_size(
                0,
                usize::from(num_tables),
                limits.max_objects,
                "TrueType cmap records",
            )?;
            let directory_end = 4 + usize::from(num_tables) * 8;
            if directory_end > table.len() {
                return Err(PdfError::FontError(
                    "Truncated cmap encoding directory".into(),
                ));
            }
            let mut cmap_records = Vec::new();
            cmap_records
                .try_reserve_exact(usize::from(num_tables))
                .map_err(|_| {
                    PdfError::ResourceLimit("TrueType cmap directory allocation".into())
                })?;
            for _i in 0..num_tables {
                let platform_id = reader.read_be_u16()?;
                let encoding_id = reader.read_be_u16()?;
                let sub_offset = reader.read_be_u32()?;
                if (sub_offset as usize) < directory_end || sub_offset as usize >= table.len() {
                    return Err(PdfError::FontError("Invalid cmap subtable offset".into()));
                }
                cmap_records.push((platform_id, encoding_id, sub_offset));
            }

            let mut cmaps = Vec::new();
            cmaps
                .try_reserve_exact(usize::from(num_tables))
                .map_err(|_| PdfError::ResourceLimit("TrueType cmap allocation".into()))?;
            let mut parsed_bytes = 0;
            for (platform_id, encoding_id, offset) in cmap_records {
                reader.reset_offset(offset as usize);
                let format = reader.read_be_u16()?;
                let length = if matches!(format, 8 | 10 | 12 | 13) {
                    let _reserved = reader.read_be_u16()?;
                    usize::try_from(reader.read_be_u32()?)
                        .map_err(|_| PdfError::FontError("cmap length overflow".into()))?
                } else if format == 14 {
                    usize::try_from(reader.read_be_u32()?)
                        .map_err(|_| PdfError::FontError("cmap length overflow".into()))?
                } else {
                    usize::from(reader.read_be_u16()?)
                };
                let minimum = match format {
                    0 | 2 | 4 => 6,
                    6 | 14 => 10,
                    8 => 8208,
                    10 => 20,
                    12 | 13 => 16,
                    _ => {
                        return Err(PdfError::UnsupportedFeature(format!(
                            "TrueType cmap format {format}"
                        )));
                    }
                };
                if length < minimum
                    || (offset as usize)
                        .checked_add(length)
                        .is_none_or(|end| end > table.len())
                {
                    return Err(PdfError::FontError(
                        "cmap subtable exceeds declared table".into(),
                    ));
                }
                crate::filter::check_output_size(
                    parsed_bytes,
                    length,
                    limits.max_decoded_bytes,
                    "TrueType cumulative cmap bytes",
                )?;
                parsed_bytes += length;
                if !matches!(format, 0 | 4 | 12) {
                    continue;
                }
                let cmap = TCmap::try_new(platform_id, encoding_id, offset, &mut reader)?;
                cmaps.push(cmap);
            }

            if num_tables != 0 && cmaps.is_empty() {
                return Err(PdfError::UnsupportedFeature(
                    "Font has no supported cmap subtable".into(),
                ));
            }

            return Ok(TrueTypeProgram {
                post_metrics,
                horizontal_metrics,
                embedding_rights,
                num_glyphs,
                cmaps,
                current: None,
            });
        } else {
            return Ok(TrueTypeProgram {
                post_metrics,
                horizontal_metrics,
                embedding_rights,
                num_glyphs,
                cmaps: Vec::new(),
                current: None,
            });
        }
    }
    pub fn num_cmap(&self) -> usize {
        self.cmaps.len()
    }

    pub fn num_glyphs(&self) -> Option<u16> {
        self.num_glyphs
    }

    pub fn horizontal_metrics(
        &self,
    ) -> Option<&crate::font::truetype::tables::horizontal::HorizontalMetrics> {
        self.horizontal_metrics.as_ref()
    }

    pub fn embedding_rights(
        &self,
    ) -> Option<crate::font::truetype::tables::os2::FontEmbeddingRights> {
        self.embedding_rights
    }

    pub fn post_metrics(&self) -> Option<crate::font::truetype::tables::post::PostMetrics> {
        self.post_metrics
    }

    pub fn validate_outline_embedding(
        &self,
        subset: bool,
        editable_document: bool,
    ) -> PdfResult<()> {
        self.embedding_rights
            .ok_or_else(|| {
                PdfError::FontError("font embedding requires OS/2 rights information".into())
            })?
            .validate_outline_embedding(subset, editable_document)
    }

    /// Unicode lookup for font construction. Missing mappings return None;
    /// malformed mapped GIDs and missing maxp are errors, never truncated.
    pub fn unicode_glyph_id(&self, character: char) -> PdfResult<Option<u16>> {
        let count = self
            .num_glyphs
            .ok_or_else(|| PdfError::FontError("Unicode font construction requires maxp".into()))?;
        let cmap = [(3, 10), (0, 4), (3, 1), (0, 3), (0, 2), (0, 1), (0, 0)]
            .into_iter()
            .find_map(|(platform, encoding)| self.get_cmap(platform, encoding))
            .ok_or_else(|| PdfError::FontError("Font has no supported Unicode cmap".into()))?;
        let gid = cmap.get_gid_u32(character as u32);
        if gid == 0 {
            return Ok(None);
        }
        if gid >= u32::from(count) {
            return Err(PdfError::FontError(
                "Unicode cmap GID exceeds maxp numGlyphs".into(),
            ));
        }
        Ok(Some(gid as u16))
    }
    pub fn get_cmap(&self, platform_id: u16, encoding_id: u16) -> Option<&TCmap> {
        for cmap in self.cmaps.iter() {
            if cmap.platform_id() == platform_id && cmap.encoding_id() == encoding_id {
                return Some(cmap);
            }
        }
        None
    }
    pub fn first_cmap(&self) -> Option<&TCmap> {
        self.cmaps.first()
    }

    pub fn selct_charmap(&mut self, is_symbolic: bool) {
        self.current = None;
        if is_symbolic {
            for (i, cmap) in self.cmaps.iter().enumerate() {
                if cmap.platform_id() == 3 && cmap.encoding_id() == 0 {
                    self.current = Some(i);
                    return;
                }
            }
        }
        // Prefer full-repertoire Unicode over BMP when both are available.
        for (platform, encoding) in [(3, 10), (0, 4)] {
            if let Some(index) = self
                .cmaps
                .iter()
                .position(|cmap| cmap.platform_id() == platform && cmap.encoding_id() == encoding)
            {
                self.current = Some(index);
                return;
            }
        }
        for (i, cmap) in self.cmaps.iter().enumerate() {
            if (cmap.platform_id() == 3 && matches!(cmap.encoding_id(), 1 | 10))
                || (cmap.platform_id() == 0 && cmap.encoding_id() <= 4)
            {
                self.current = Some(i);
                return;
            }
        }

        for (i, cmap) in self.cmaps.iter().enumerate() {
            if cmap.platform_id() == 1 && cmap.encoding_id() == 0 {
                self.current = Some(i);
                return;
            }
        }
        if self.cmaps.len() > 0 {
            self.current = Some(0);
        }
    }

    pub fn current_cmap(&self) -> Option<&TCmap> {
        if let Some(index) = self.current {
            return self.cmaps.get(index);
        }
        None
    }

    pub fn has_unicode_cmap(&self) -> bool {
        for cmap in self.cmaps.iter() {
            if (cmap.platform_id() == 3 && matches!(cmap.encoding_id(), 1 | 10))
                || (cmap.platform_id() == 0 && cmap.encoding_id() <= 4)
            {
                return true;
            }
        }
        false
    }
    pub fn has_mac_roman_cmap(&self) -> bool {
        for cmap in self.cmaps.iter() {
            if cmap.platform_id() == 1 && cmap.encoding_id() == 0 {
                return true;
            }
        }
        false
    }
    pub fn has_mac_unicode_cmap(&self) -> bool {
        for cmap in self.cmaps.iter() {
            if cmap.platform_id() == 0 && cmap.encoding_id() < 4 {
                return true;
            }
        }
        false
    }

    pub fn has_ms_symbol_cmap(&self) -> bool {
        for cmap in self.cmaps.iter() {
            if cmap.platform_id() == 3 && cmap.encoding_id() == 0 {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn composite_sfnt_outline_normalizes_after_expansion() {
        use crate::{
            font::truetype::tables::glyf::GlyphOutlineCommand as C, parser::options::Limits,
        };
        let mut head = vec![0; 54];
        head[18..20].copy_from_slice(&1000_u16.to_be_bytes());
        head[50..52].copy_from_slice(&1_i16.to_be_bytes());
        let mut maxp = vec![0; 32];
        maxp[..4].copy_from_slice(&0x10000_u32.to_be_bytes());
        maxp[4..6].copy_from_slice(&3_u16.to_be_bytes());
        let mut glyf: Vec<u8> = [-1_i16, 0, 0, 40, 70]
            .into_iter()
            .flat_map(i16::to_be_bytes)
            .collect();
        for value in [11_u16, 1, 3, 5, 8192] {
            glyf.extend(value.to_be_bytes());
        }
        let first_end = glyf.len() as u32;
        glyf.extend([
            0, 1, 0, 0, 0, 0, 0, 30, 0, 60, 0, 2, 0, 0, 0x3f, 2, 10, 10, 10, 20, 20, 20,
        ]);
        let last_end = glyf.len() as u32;
        let loca = [0_u32, first_end, last_end, last_end]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect::<Vec<_>>();
        let tables = [
            (b"glyf", glyf),
            (b"head", head),
            (b"loca", loca),
            (b"maxp", maxp),
        ];
        let mut data = vec![0, 1, 0, 0, 0, 4, 0, 64, 0, 2, 0, 0];
        let mut offset = 76_u32;
        for (tag, bytes) in &tables {
            data.extend(*tag);
            data.extend(0_u32.to_be_bytes());
            data.extend(offset.to_be_bytes());
            data.extend((bytes.len() as u32).to_be_bytes());
            offset += ((bytes.len() + 3) & !3) as u32;
        }
        for (_, bytes) in tables {
            data.extend(bytes);
            while data.len() % 4 != 0 {
                data.push(0);
            }
        }
        let limits = Limits::default();
        assert_eq!(
            TrueTypeProgram::checked_glyph_outline(&data, 0, limits).unwrap(),
            [
                C::Move([0.008, 0.015]),
                C::Line([0.013, 0.025]),
                C::Line([0.018, 0.035]),
                C::Close
            ]
        );
        assert_eq!(
            TrueTypeProgram::checked_glyph_outline(&data, 1, limits).unwrap(),
            TrueTypeProgram::checked_simple_glyph_outline(&data, 1, limits).unwrap()
        );
        assert!(
            TrueTypeProgram::checked_glyph_outline(&data, 2, limits)
                .unwrap()
                .is_empty()
        );
        assert!(TrueTypeProgram::checked_glyph_outline(&data, 3, limits).is_err());
        assert!(TrueTypeProgram::checked_simple_glyph_outline(&data, 0, limits).is_err());
        assert!(
            TrueTypeProgram::checked_glyph_outline(
                &data,
                0,
                Limits {
                    max_depth: 1,
                    ..limits
                }
            )
            .is_err()
        );
        // PDF character 65 maps to composite GID 0; no Unicode cmap is present.
        use crate::{
            document::{PdfDocument, resource_manager::ResourceKind},
            objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
            page::page_object::PageObject,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let program = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), data.clone()).unwrap(),
            ))
            .unwrap();
        let dictionary = format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /CompositeTest /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /CompositeTest /CIDToGIDMap << >> /FontDescriptor << /FontFile2 {} 0 R >> >>] >>",
            program.obj_num()
        );
        // Use an explicit stream map, not malformed-map fallback.
        let mut map = vec![0; 134];
        map[133] = 2;
        let map_id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), map).unwrap(),
            ))
            .unwrap();
        let dictionary = dictionary.replace(
            "/CIDToGIDMap << >>",
            &format!("/CIDToGIDMap {} 0 R", map_id.obj_num()),
        );
        let mut parser = crate::parser::syntax::SyntaxParser::new(
            crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                dictionary.into_bytes(),
            ))
            .unwrap(),
        );
        let font_id = doc.try_insert_object(parser.get_object().unwrap()).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Font, font_id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{name} 10 Tf 1 0 0 1 20 40 Tm <00410042> Tj ET").into_bytes(),
        )
        .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        for document in [&doc, &reopened] {
            let list = document.get_page(0).unwrap().display_list().unwrap();
            let text = list
                .objects()
                .iter()
                .find_map(|o| {
                    if let PageObject::Text(t) = o {
                        Some(t)
                    } else {
                        None
                    }
                })
                .unwrap();
            let font = text.state.text_state.font.as_ref().unwrap();
            assert_eq!(
                font.glyph_outline(text.items[0].charcode(), limits)
                    .unwrap(),
                TrueTypeProgram::checked_glyph_outline(&data, 0, limits).unwrap()
            );
            assert!(
                font.simple_glyph_outline(text.items[0].charcode(), limits)
                    .is_err()
            );
            let commands = text.character_outline_display(0, limits).unwrap();
            let C::Move(point) = commands[0] else {
                panic!("missing move")
            };
            let origin = text.items[0].pos();
            assert!((point[0] - (f64::from(origin.x()) + 0.08)).abs() < 1e-12);
            assert!((point[1] - (f64::from(origin.y()) - 0.15)).abs() < 1e-12);
            assert_eq!(
                text.character_outline_location_display(0, point, limits)
                    .unwrap(),
                crate::font::truetype::tables::outline_hit::OutlineLocation::Boundary
            );
            assert!(
                text.character_outline_display(1, limits)
                    .unwrap()
                    .is_empty()
            );
        }
    }
    #[test]
    fn simple_outline_from_sfnt_normalizes_by_units_per_em() {
        use super::TrueTypeProgram;
        use crate::{font::truetype::tables::glyf::GlyphOutlineCommand, parser::options::Limits};
        fn fixture(units: u16, composite: bool) -> Vec<u8> {
            let mut head = vec![0; 54];
            head[18..20].copy_from_slice(&units.to_be_bytes());
            let mut maxp = vec![0; 32];
            maxp[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
            maxp[4..6].copy_from_slice(&2_u16.to_be_bytes());
            let mut glyf = vec![
                0, 1, 0, 0, 0, 0, 0, 30, 0, 60, 0, 2, 0, 0, 0x3f, 2, 10, 10, 10, 20, 20, 20,
            ];
            if composite {
                glyf[..2].copy_from_slice(&(-1_i16).to_be_bytes());
            }
            let tables = [
                (b"glyf", glyf),
                (b"head", head),
                (b"loca", vec![0, 0, 0, 11, 0, 11]),
                (b"maxp", maxp),
            ];
            let mut data = vec![0, 1, 0, 0, 0, 4, 0, 64, 0, 2, 0, 0];
            let mut offset = 76_u32;
            for (tag, bytes) in &tables {
                data.extend_from_slice(*tag);
                data.extend_from_slice(&0_u32.to_be_bytes());
                data.extend_from_slice(&offset.to_be_bytes());
                data.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                offset += ((bytes.len() + 3) & !3) as u32;
            }
            for (_, bytes) in tables {
                data.extend(bytes);
                while data.len() % 4 != 0 {
                    data.push(0);
                }
            }
            data
        }
        // PDF source code 65 must use CIDToGIDMap -> GID 0, not GID 65
        // or an invented Unicode cmap lookup; code 66 maps to the empty GID 1.
        {
            use crate::{
                document::{PdfDocument, resource_manager::ResourceKind},
                objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
                page::page_object::PageObject,
            };
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let program_id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(PdfDict::default(), fixture(1000, false)).unwrap(),
                ))
                .unwrap();
            let mut map = vec![0; 134];
            map[133] = 1;
            let map_id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(PdfDict::default(), map).unwrap(),
                ))
                .unwrap();
            let dictionary = format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /OutlineTest /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /OutlineTest /CIDToGIDMap {} 0 R /FontDescriptor << /FontFile2 {} 0 R >> >>] >>",
                map_id.obj_num(),
                program_id.obj_num()
            );
            let mut parser = crate::parser::syntax::SyntaxParser::new(
                crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                    dictionary.into_bytes(),
                ))
                .unwrap(),
            );
            let font_id = doc.try_insert_object(parser.get_object().unwrap()).unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Font, font_id)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("BT /{name} 12 Tf 50 Tz 3 Ts 1 0.25 -0.5 1 20 40 Tm <004100410042> Tj ET")
                    .into_bytes(),
            )
            .unwrap();
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            for doc in [&doc, &reopened] {
                let list = doc.get_page(0).unwrap().display_list().unwrap();
                let text = list
                    .objects()
                    .iter()
                    .find_map(|object| {
                        if let PageObject::Text(text) = object {
                            Some(text)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                let font = text.state.text_state.font.as_ref().unwrap();
                assert_eq!(
                    font.simple_glyph_outline(text.items[0].charcode(), Limits::default())
                        .unwrap(),
                    TrueTypeProgram::checked_simple_glyph_outline(
                        &fixture(1000, false),
                        0,
                        Limits::default()
                    )
                    .unwrap()
                );
                assert!(
                    font.simple_glyph_outline(text.items[2].charcode(), Limits::default())
                        .unwrap()
                        .is_empty()
                );
                for index in [0, 1] {
                    let position = text
                        .character_outline_display(index, Limits::default())
                        .unwrap();
                    let GlyphOutlineCommand::Move(point) = position[0] else {
                        panic!("missing move");
                    };
                    assert_eq!(
                        text.character_outline_location_display(index, point, Limits::default())
                            .unwrap(),
                        crate::font::truetype::tables::outline_hit::OutlineLocation::Boundary
                    );
                    let bounds = text
                        .character_outline_display_bounds(index, Limits::default())
                        .unwrap()
                        .unwrap();
                    let origin = text.items[index].pos();
                    let expected = [
                        f64::from(origin.x()) - 0.18,
                        f64::from(origin.y()) - 0.765,
                        f64::from(origin.x()) - 0.06,
                        f64::from(origin.y()) - 0.255,
                    ];
                    assert!(
                        bounds
                            .iter()
                            .zip(expected)
                            .all(|(actual, expected)| (actual - expected).abs() < 1e-6)
                    );
                    let outline = text
                        .character_outline_display(index, Limits::default())
                        .unwrap();
                    for (command, (x, y)) in
                        outline[..3]
                            .iter()
                            .zip([(0.01, 0.02), (0.02, 0.04), (0.03, 0.06)])
                    {
                        let point = match command {
                            GlyphOutlineCommand::Move(point) | GlyphOutlineCommand::Line(point) => {
                                point
                            }
                            _ => panic!("unexpected outline command"),
                        };
                        let expected_x = f64::from(text.items[index].pos().x()) + x * 6. - y * 6.;
                        let expected_y = f64::from(text.items[index].pos().y()) - x * 1.5 - y * 12.;
                        assert!(
                            (point[0] - expected_x).abs() < 1e-6
                                && (point[1] - expected_y).abs() < 1e-6
                        );
                    }
                }
                assert_ne!(
                    text.character_outline_display(0, Limits::default())
                        .unwrap(),
                    text.character_outline_display(1, Limits::default())
                        .unwrap()
                );
                assert!(
                    text.character_outline_display(2, Limits::default())
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    text.character_outline_display_bounds(2, Limits::default())
                        .unwrap(),
                    None
                );
                assert!(
                    text.character_outline_display(3, Limits::default())
                        .is_err()
                );
            }
        }
        for units in [16, 1000, 2048, 16384] {
            let data = fixture(units, false);
            let outline =
                TrueTypeProgram::checked_simple_glyph_outline(&data, 0, Limits::default()).unwrap();
            let scale = f64::from(units);
            assert_eq!(
                outline,
                [
                    GlyphOutlineCommand::Move([10. / scale, 20. / scale]),
                    GlyphOutlineCommand::Line([20. / scale, 40. / scale]),
                    GlyphOutlineCommand::Line([30. / scale, 60. / scale]),
                    GlyphOutlineCommand::Close
                ]
            );
            assert!(
                TrueTypeProgram::checked_simple_glyph_outline(&data, 1, Limits::default())
                    .unwrap()
                    .is_empty()
            );
            assert!(
                TrueTypeProgram::checked_simple_glyph_outline(&data, 2, Limits::default()).is_err()
            );
        }
        for units in [0, 15, 16385] {
            assert!(
                TrueTypeProgram::checked_simple_glyph_outline(
                    &fixture(units, false),
                    0,
                    Limits::default()
                )
                .is_err()
            );
        }
        assert!(
            TrueTypeProgram::checked_simple_glyph_outline(
                &fixture(1000, true),
                0,
                Limits::default()
            )
            .is_err()
        );
    }
    use crate::font::builtin_font::load_builtin_font_data;

    use super::TrueTypeProgram;

    #[test]
    fn os2_rights_are_loaded_without_preventing_reading_restricted_fonts() {
        fn font(flags: u16) -> Vec<u8> {
            let mut data = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
            data.extend_from_slice(b"OS/2");
            for value in [0u32, 28, 10] {
                data.extend_from_slice(&value.to_be_bytes());
            }
            data.extend_from_slice(&[0, 4, 0, 0, 0, 0, 0, 0]);
            data.extend_from_slice(&flags.to_be_bytes());
            data
        }
        let editable = TrueTypeProgram::try_new(&font(8)).unwrap();
        assert_eq!(editable.embedding_rights().unwrap().raw_fs_type(), 8);
        assert!(editable.validate_outline_embedding(false, true).is_ok());
        let restricted = TrueTypeProgram::try_new(&font(2)).unwrap();
        assert!(restricted.validate_outline_embedding(false, false).is_err());
        let missing = TrueTypeProgram::try_new(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(missing.embedding_rights().is_none());
        assert!(missing.validate_outline_embedding(false, true).is_err());
    }

    #[test]
    fn maxp_count_checks_version_size_and_zero_count() {
        fn font(version: u32, count: u16, length: u32) -> Vec<u8> {
            let mut data = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
            data.extend_from_slice(b"maxp");
            for word in [0u32, 28, length] {
                data.extend_from_slice(&word.to_be_bytes());
            }
            data.extend_from_slice(&version.to_be_bytes());
            data.extend_from_slice(&count.to_be_bytes());
            data.resize(28 + length as usize, 0);
            data
        }
        for (version, length) in [(0x5000, 6), (0x10000, 32)] {
            assert_eq!(
                TrueTypeProgram::try_new(&font(version, 103, length))
                    .unwrap()
                    .num_glyphs(),
                Some(103)
            );
            assert!(TrueTypeProgram::try_new(&font(version, 0, length)).is_err());
            assert!(TrueTypeProgram::try_new(&font(version, 103, length - 1)).is_err());
        }
        assert!(TrueTypeProgram::try_new(&font(0x20000, 103, 32)).is_err());
    }

    #[test]
    fn unicode_lookup_checks_selected_mapping_against_maxp() {
        use crate::font::truetype::{reader::TrueTypeReader, tables::cmap::TCmap};
        let mut table = vec![0, 12, 0, 0];
        for value in [28u32, 0, 1, 0x20000, 0x20002, 100] {
            table.extend_from_slice(&value.to_be_bytes());
        }
        let mut program = TrueTypeProgram {
            post_metrics: None,
            horizontal_metrics: None,
            embedding_rights: None,
            num_glyphs: Some(103),
            cmaps: vec![TCmap::try_new(3, 10, 0, &mut TrueTypeReader::new(&table)).unwrap()],
            current: None,
        };
        assert_eq!(
            program
                .unicode_glyph_id(char::from_u32(0x20002).unwrap())
                .unwrap(),
            Some(102)
        );
        assert_eq!(program.unicode_glyph_id('A').unwrap(), None);
        program.num_glyphs = Some(102);
        assert!(
            program
                .unicode_glyph_id(char::from_u32(0x20002).unwrap())
                .is_err()
        );
        program.num_glyphs = None;
        assert!(program.unicode_glyph_id('A').is_err());
    }

    #[test]
    fn full_unicode_cmap_is_loaded_and_selected_without_losing_symbol_priority() {
        use crate::font::truetype::{reader::TrueTypeReader, tables::cmap::TCmap};
        let mut data = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        data.extend_from_slice(b"cmap");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&40u32.to_be_bytes());
        data.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10]);
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(&[0, 12, 0, 0]);
        for value in [28u32, 0, 1, 0x20000, 0x20002, 100] {
            data.extend_from_slice(&value.to_be_bytes());
        }
        let mut program = TrueTypeProgram::try_new(&data).unwrap();
        assert!(program.has_unicode_cmap());
        program.selct_charmap(false);
        assert_eq!(program.current_cmap().unwrap().get_gid_u32(0x20001), 101);
        let mut legacy = vec![0, 0, 1, 6, 0, 0];
        legacy.extend_from_slice(&[0; 256]);
        for encoding in [1, 0] {
            program.cmaps.insert(
                0,
                TCmap::try_new(3, encoding, 0, &mut TrueTypeReader::new(&legacy)).unwrap(),
            );
        }
        program.selct_charmap(false);
        assert_eq!(program.current_cmap().unwrap().encoding_id(), 10);
        program.selct_charmap(true);
        assert_eq!(program.current_cmap().unwrap().encoding_id(), 0);
    }

    #[test]
    fn duplicate_cmap_records_share_cumulative_parse_budget() {
        use crate::{error::PdfError, parser::options::Limits};
        let mut data = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
        data.extend_from_slice(b"cmap");
        data.extend_from_slice(&0_u32.to_be_bytes());
        data.extend_from_slice(&28_u32.to_be_bytes());
        data.extend_from_slice(&290_u32.to_be_bytes());
        data.extend_from_slice(&[0, 0, 0, 3]);
        for encoding in 0..3_u16 {
            data.extend_from_slice(&3_u16.to_be_bytes());
            data.extend_from_slice(&encoding.to_be_bytes());
            data.extend_from_slice(&28_u32.to_be_bytes());
        }
        data.extend_from_slice(&[0, 0, 1, 6, 0, 0]);
        data.extend_from_slice(&[0; 256]);
        let limits = Limits {
            max_decoded_bytes: 786,
            max_objects: 3,
            ..Default::default()
        };
        assert_eq!(
            TrueTypeProgram::try_new_with_limits(&data, limits)
                .unwrap()
                .num_cmap(),
            3
        );
        for limits in [
            Limits {
                max_decoded_bytes: 785,
                ..limits
            },
            Limits {
                max_objects: 2,
                ..limits
            },
            Limits {
                max_decoded_bytes: data.len() - 1,
                ..limits
            },
        ] {
            assert!(matches!(
                TrueTypeProgram::try_new_with_limits(&data, limits),
                Err(PdfError::ResourceLimit(_))
            ));
        }
    }

    #[test]
    fn charmap_selection_prefers_symbol_or_unicode_and_resets() {
        use crate::font::truetype::{reader::TrueTypeReader, tables::cmap::TCmap};
        let mut table = vec![0, 0, 1, 6, 0, 0];
        table.extend_from_slice(&[0; 256]);
        let mut program = TrueTypeProgram {
            post_metrics: None,
            horizontal_metrics: None,
            embedding_rights: None,
            num_glyphs: None,
            cmaps: Vec::new(),
            current: None,
        };
        for (platform, encoding) in [(1, 0), (3, 1), (3, 0)] {
            program.cmaps.push(
                TCmap::try_new(platform, encoding, 0, &mut TrueTypeReader::new(&table)).unwrap(),
            );
        }
        program.selct_charmap(true);
        assert_eq!(program.current_cmap().unwrap().encoding_id(), 0);
        assert_eq!(program.current_cmap().unwrap().platform_id(), 3);
        program.selct_charmap(false);
        assert_eq!(program.current_cmap().unwrap().encoding_id(), 1);
        program.cmaps.pop();
        program.selct_charmap(true);
        assert_eq!(program.current_cmap().unwrap().encoding_id(), 1);
        program.cmaps.pop();
        program.selct_charmap(false);
        assert_eq!(program.current_cmap().unwrap().platform_id(), 1);
        program.cmaps.clear();
        program.selct_charmap(true);
        assert!(program.current_cmap().is_none());
        assert_eq!(program.current, None);
    }

    #[test]
    fn cmap_is_bounded_by_table_not_whole_font() {
        fn font(table: &[u8], declared_length: u32) -> Vec<u8> {
            let mut data = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
            data.extend_from_slice(b"cmap");
            data.extend_from_slice(&0_u32.to_be_bytes());
            data.extend_from_slice(&28_u32.to_be_bytes());
            data.extend_from_slice(&declared_length.to_be_bytes());
            data.extend_from_slice(table);
            data
        }
        let mut table = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12, 0, 0, 1, 6, 0, 0];
        table.extend_from_slice(&[0; 256]);
        assert!(TrueTypeProgram::try_new(&font(&table, table.len() as u32)).is_ok());
        for length in [0, 3, 4, 11, 12, 18, 273] {
            assert!(TrueTypeProgram::try_new(&font(&table, length)).is_err());
        }
        for offset in [0_u32, 4, 11, 274, u32::MAX] {
            let mut bad = table.clone();
            bad[8..12].copy_from_slice(&offset.to_be_bytes());
            assert!(TrueTypeProgram::try_new(&font(&bad, bad.len() as u32)).is_err());
        }
        let mut bad = table.clone();
        bad[1] = 1;
        assert!(TrueTypeProgram::try_new(&font(&bad, bad.len() as u32)).is_err());
    }

    #[test]
    fn test_truetype_program() {
        let font_data = load_builtin_font_data("TimesNewRoman").unwrap();
        // The built-in font is Type 1 PFB, not a TrueType program.
        assert!(matches!(
            TrueTypeProgram::try_new(font_data),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
    }

    #[test]
    fn optional_unsupported_cmaps_do_not_hide_supported_mapping() {
        for optional in [
            vec![0, 14, 0, 0, 0, 10, 0, 0, 0, 0],
            vec![0, 6, 0, 10, 0, 0, 0, 0, 0, 0],
        ] {
            let mut cmap = vec![0, 0, 0, 2, 0, 3, 0, 1, 0, 0, 0, 20, 0, 0, 0, 5, 0, 0, 1, 26];
            cmap.extend_from_slice(&[0, 0, 1, 6, 0, 0]);
            cmap.extend_from_slice(&[0; 256]);
            cmap[26 + usize::from(b'A')] = 7;
            cmap.extend_from_slice(&optional);
            let mut font = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
            font.extend_from_slice(b"cmap");
            font.extend_from_slice(&0_u32.to_be_bytes());
            font.extend_from_slice(&28_u32.to_be_bytes());
            font.extend_from_slice(&(cmap.len() as u32).to_be_bytes());
            font.extend_from_slice(&cmap);
            let mut program = TrueTypeProgram::try_new(&font).unwrap();
            assert_eq!(program.num_cmap(), 1);
            program.selct_charmap(false);
            assert_eq!(program.current_cmap().unwrap().get_gid(65), 7);
            font.pop();
            assert!(TrueTypeProgram::try_new(&font).is_err());
        }
    }

    #[test]
    fn sfnt_horizontal_metrics_are_loaded_from_declared_tables() {
        // Metadata fixture only: this is not a renderable font.
        let mut head = vec![0; 54];
        head[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5f0f3cf5_u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000_u16.to_be_bytes());
        let mut hhea = vec![0; 36];
        hhea[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
        hhea[34..36].copy_from_slice(&1_u16.to_be_bytes());
        let mut maxp = vec![0; 32];
        maxp[..4].copy_from_slice(&0x00010000_u32.to_be_bytes());
        maxp[4..6].copy_from_slice(&4_u16.to_be_bytes());
        let mut hmtx = vec![0; 10];
        hmtx[..2].copy_from_slice(&500_u16.to_be_bytes());
        let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12, 0, 0, 1, 6, 0, 0];
        cmap.extend_from_slice(&[0; 256]);
        cmap[18 + usize::from(b'A')] = 1;
        let mut post = vec![0; 32];
        post[..4].copy_from_slice(&0x00030000_u32.to_be_bytes());
        post[4..8].copy_from_slice(&(-819200_i32).to_be_bytes());
        let tables = [
            (b"OS/2", vec![0; 10]),
            (b"cmap", cmap),
            (b"glyf", vec![0; 10]),
            (b"head", head),
            (b"hhea", hhea),
            (b"hmtx", hmtx),
            (b"loca", vec![0, 0, 0, 5, 0, 5, 0, 5, 0, 5]),
            (b"maxp", maxp),
            (b"post", post),
        ];
        let mut data = vec![0, 1, 0, 0, 0, 9, 0, 128, 0, 3, 0, 16];
        let mut offset = 156_u32;
        for (tag, bytes) in &tables {
            data.extend_from_slice(*tag);
            data.extend_from_slice(&0_u32.to_be_bytes());
            data.extend_from_slice(&offset.to_be_bytes());
            data.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            offset += ((bytes.len() + 3) & !3) as u32;
        }
        for (_, bytes) in tables {
            data.extend_from_slice(&bytes);
            while data.len() % 4 != 0 {
                data.push(0);
            }
        }
        let program = TrueTypeProgram::try_new(&data).unwrap();
        let limits = crate::parser::options::Limits::default();
        assert_eq!(program.post_metrics().unwrap().italic_angle(), -12.5);
        let encoding =
            crate::font::cid_encoding::UnicodeCidEncoding::from_text(&program, "AA", limits)
                .unwrap();
        assert_eq!(encoding.encode_text("AA").unwrap(), [0, 1, 0, 1]);
        assert_eq!(encoding.cid_to_gid_map().unwrap(), [0, 0, 0, 1]);
        assert_eq!(encoding.widths().collect::<Vec<_>>(), [(1, 500.0)]);
        assert!(
            crate::font::cid_encoding::UnicodeCidEncoding::from_text(&program, "B", limits)
                .is_err()
        );
        let locations = TrueTypeProgram::checked_glyph_locations(&data, limits).unwrap();
        assert_eq!(locations.glyph_data(0).unwrap(), &[0; 10]);
        assert!(locations.glyph_data(3).unwrap().is_empty());
        let mut cff = data.clone();
        cff[..4].copy_from_slice(b"OTTO");
        assert!(TrueTypeProgram::checked_glyph_locations(&cff, limits).is_err());
        let mut missing = data.clone();
        missing[44..48].copy_from_slice(b"xxxx");
        assert!(TrueTypeProgram::checked_glyph_locations(&missing, limits).is_err());
        let document = crate::document::PdfDocument::new().unwrap();
        document.add_page(0).unwrap();
        let options = || crate::document::embedded_font::TrueTypeEmbeddingOptions {
            postscript_name: "RzpdfTestFont".into(),
            cap_height: 700.0,
            stem_v: 80.0,
            serif: false,
        };
        let revision = document.revision();
        assert!(
            document
                .resources()
                .embed_truetype_font(0, data.clone(), "B", options())
                .is_err()
        );
        assert_eq!(document.revision(), revision);
        let mut restricted = data.clone();
        restricted[164..166].copy_from_slice(&2_u16.to_be_bytes());
        assert!(
            document
                .resources()
                .embed_truetype_font(0, restricted, "A", options())
                .is_err()
        );
        assert_eq!(document.revision(), revision);
        let limited =
            crate::document::PdfDocument::new_with_options(crate::parser::options::ParseOptions {
                limits: crate::parser::options::Limits {
                    max_objects: 9,
                    ..limits
                },
                ..Default::default()
            })
            .unwrap();
        limited.add_page(0).unwrap();
        let snapshot = || {
            (1..=4)
                .map(|id| {
                    limited
                        .resolve_object(crate::objects::object_id::ObjectId::new(id, 0))
                        .unwrap()
                })
                .collect::<Vec<_>>()
        };
        let before = snapshot();
        assert!(
            limited
                .resources()
                .embed_truetype_font(0, data.clone(), "A", options())
                .is_err()
        );
        assert_eq!(snapshot(), before);
        assert_eq!(
            limited
                .try_insert_object(crate::objects::PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            5
        );
        let registered = document
            .resources()
            .embed_truetype_font(0, data.clone(), "AA", options())
            .unwrap();
        assert_eq!(registered.resource_name, "F1");
        assert_eq!(
            registered.encoding().encode_text("AA").unwrap(),
            [0, 1, 0, 1]
        );
        document.add_page(1).unwrap();
        document
            .resources()
            .register_standard_font(
                1,
                crate::document::resource_manager::StandardFont::Helvetica,
            )
            .unwrap();
        assert_eq!(registered.register_on_page(&document, 1).unwrap(), "F2");
        assert_eq!(registered.register_on_page(&document, 1).unwrap(), "F2");
        let foreign = crate::document::PdfDocument::new().unwrap();
        foreign.add_page(0).unwrap();
        let foreign_revision = foreign.revision();
        assert!(registered.register_on_page(&foreign, 0).is_err());
        assert_eq!(foreign.revision(), foreign_revision);
        assert!(registered.register_on_page(&document, 2).is_err());
        // Exercise CID numbering with this metadata fixture, not visual glyphs.
        let mut numbered_font = data.clone();
        let cmap_offset = u32::from_be_bytes(numbered_font[36..40].try_into().unwrap()) as usize;
        numbered_font[cmap_offset + 18 + usize::from(b'1')] = 1;
        numbered_font[cmap_offset + 18 + usize::from(b'2')] = 1;
        let numbered = crate::document::PdfDocument::new().unwrap();
        numbered.add_page(0).unwrap();
        numbered.add_page(1).unwrap();
        let cid_font = numbered
            .resources()
            .embed_truetype_font(0, numbered_font, "12", options())
            .unwrap();
        let format = crate::document::page_overlay::PageNumberFormat {
            start: 1,
            prefix: "",
            total_separator: None,
            suffix: "",
        };
        let matrix = [1.0, 0.0, 0.0, 1.0, 48.0, 48.0];
        numbered.replace_page_contents(1, b"Q".to_vec()).unwrap();
        let snapshot = || {
            (1..=13)
                .map(|id| {
                    numbered
                        .resolve_object(crate::objects::object_id::ObjectId::new(id, 0))
                        .unwrap()
                })
                .collect::<Vec<_>>()
        };
        let before = snapshot();
        assert!(
            numbered
                .number_pages_cid(&[0, 1], &format, &cid_font, 12.0, matrix, [0.0; 3])
                .is_err()
        );
        assert_eq!(snapshot(), before);
        let revision = foreign.revision();
        assert!(
            foreign
                .number_pages_cid(&[0], &format, &cid_font, 12.0, matrix, [0.0; 3])
                .is_err()
        );
        assert_eq!(foreign.revision(), revision);
        numbered.replace_page_contents(1, Vec::new()).unwrap();
        numbered
            .number_pages_cid(&[1, 0], &format, &cid_font, 12.0, matrix, [0.0; 3])
            .unwrap();
        let revision = numbered.revision();
        assert!(
            numbered
                .watermark_pages_cid(&[0, 1], "A", &cid_font, 20.0, matrix, [0.5; 3])
                .is_err()
        );
        assert_eq!(numbered.revision(), revision);
        numbered
            .watermark_pages_cid(&[0, 1], "12", &cid_font, 20.0, matrix, [0.5; 3])
            .unwrap();
        let result = numbered.save_to_bytes().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&result)
                .matches("/FontFile2")
                .count(),
            1
        );
        let reopened = crate::document::PdfDocument::from_bytes_with_mode(
            result,
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        let saved = document.save_to_bytes().unwrap();
        let serialized = String::from_utf8_lossy(&saved);
        assert_eq!(serialized.matches("/FontFile2").count(), 1);
        assert_eq!(serialized.matches("/CIDFontType2").count(), 1);
        for token in [
            "/CIDFontType2",
            "/FontFile2",
            "/ToUnicode",
            "/Identity-H",
            "<0001> <0041>",
        ] {
            assert!(serialized.contains(token), "{token}");
        }
        let reopened = crate::document::PdfDocument::from_bytes_with_mode(
            saved,
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        assert_eq!(
            program
                .horizontal_metrics()
                .unwrap()
                .advance_width(3)
                .unwrap(),
            500
        );
    }
}

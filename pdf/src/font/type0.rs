use std::char;
use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::cid_font::CidFont;
use crate::font::cmap::Cmap;
use crate::objects::{PdfObject, pdf_dict::PdfDict};
use crate::pdf_context::PDFContext;

use super::{CharCode, GlyphDesc, WritingMode};

#[derive(Debug, Default, Clone)]
pub struct Type0 {
    base_font: Option<String>,
    encoding: Option<Cmap>,
    to_unicode: Option<Cmap>,
    descent_font: CidFont,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn budget_construction_tracks_named_and_stream_cmaps_until_font_release() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            font::pdf_font::Font,
            objects::{pdf_name::PdfName, pdf_stream::PdfStream},
        };
        let doc = Document::new().unwrap();
        let source = b"<< /Subtype /Type0 /BaseFont /BudgetType0 /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /DW 500 >>] >>";
        let object =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(source.to_vec())).unwrap())
                .get_object()
                .unwrap();
        let mut dict = object.as_dict().unwrap().clone();
        dict.insert(
            "ToUnicode".into(),
            PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"1 beginbfchar <0041> <0041> endbfchar".to_vec(),
            )),
        );
        for (encoding, bytes) in [
            (PdfObject::PdfName(PdfName::new("Identity-H".into())), &b"\0A"[..]),
            (PdfObject::PdfStream(PdfStream::new(PdfDict::default(), b"1 begincodespacerange <00> <FF> endcodespacerange 1 begincidchar <41> 7 endcidchar".to_vec())), &b"A"[..]),
        ] {
            dict.insert("Encoding".into(), encoding);
            let probe = OperationBudget::new(2 * 1024 * 1024);
            let (font, lease) = Font::try_new_with_budget(&dict, doc.ctx(), &probe).unwrap();
            let Font::Type0(composite) = &font else { panic!("expected Type0"); };
            let characters = composite.chars(bytes).unwrap();
            assert_eq!(characters.len(), 1);
            assert_eq!(characters[0].code(), 65);
            assert_eq!(characters[0].width, 500.0);
            assert_eq!(composite.unicode_mapping(&characters[0]), Some("A"));
            assert!(lease.bytes() > composite.base_font.as_ref().unwrap().capacity());
            assert_eq!(probe.live_bytes(), lease.bytes());
            let peak = probe.peak_bytes();
            drop((font, lease));
            assert_eq!(probe.live_bytes(), 0);
            let subtype = doc.try_insert_object(PdfObject::PdfName(PdfName::new("Type0".into()))).unwrap();
            let mut indirect = dict.clone();
            indirect.insert("Subtype".into(), PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::from_id(subtype)));
            let (font, lease) = Font::try_new_with_budget(&indirect, doc.ctx(), &probe).unwrap();
            assert!(matches!(font, Font::Type0(_)));
            assert!(lease.bytes() > 0);
            assert_eq!(probe.live_bytes(), lease.bytes());
            drop((font, lease));
            assert_eq!(probe.live_bytes(), 0);
            assert!(matches!(Font::try_new(&indirect, doc.ctx()).unwrap(), Font::Type0(_)));
            let pool = OperationBudget::new(peak);
            let neighbor = pool.reserve(1).unwrap();
            assert!(matches!(Font::try_new_with_budget(&dict, doc.ctx(), &pool), Err(PdfError::ResourceLimit(_))));
            assert_eq!(pool.live_bytes(), 1);
            drop(neighbor);
            drop(Font::try_new_with_budget(&dict, doc.ctx(), &pool).unwrap());
            assert_eq!(pool.live_bytes(), 0);
            let mut invalid = dict.clone();
            invalid.insert("ToUnicode".into(), PdfObject::PdfNull);
            assert!(Font::try_new_with_budget(&invalid, doc.ctx(), &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn reverse_composite_encoding_preserves_one_to_four_byte_source_codes() {
        let encoding = Cmap::try_new(b"4 begincodespacerange <21> <7F> <8000> <80FF> <810000> <81FFFF> <82000000> <82FFFFFF> endcodespacerange 5 begincidchar <21> 7 <22> 8 <8001> 9 <810001> 10 <82000001> 11 endcidchar".to_vec()).unwrap();
        let unicode = Cmap::try_new(b"4 begincodespacerange <21> <7F> <8000> <80FF> <810000> <81FFFF> <82000000> <82FFFFFF> endcodespacerange 5 beginbfchar <21> <0061> <22> <0061> <8001> <00620063> <810001> <D83DDE00> <82000001> <4E2D> endbfchar".to_vec()).unwrap();
        let font = Type0 {
            encoding: Some(encoding),
            to_unicode: Some(unicode),
            ..Default::default()
        };
        let limits = crate::parser::options::Limits::default();
        let maximum = 20
            + ("abc😀中".len() + 1) * std::mem::size_of::<Option<(u32, u8, usize)>>()
            + 8 * std::mem::size_of::<(u32, u8, &str)>();
        let budget = crate::budget::OperationBudget::new(maximum);
        let codes = font
            .encode_text_with_budget("abc😀中", limits, &budget)
            .unwrap();
        assert_eq!(
            codes.as_slice(),
            &[0x21, 0x80, 1, 0x81, 0, 1, 0x82, 0, 0, 1]
        );
        assert_eq!(budget.live_bytes(), codes.capacity());
        let neighbor = budget.reserve(maximum - budget.live_bytes()).unwrap();
        assert!(font.encode_text_with_budget("a", limits, &budget).is_err());
        drop(neighbor);
        drop(codes);
        assert_eq!(budget.live_bytes(), 0);
        let small = crate::budget::OperationBudget::new(maximum - 1);
        assert!(
            font.encode_text_with_budget("abc😀中", limits, &small)
                .is_err()
        );
        assert_eq!(small.live_bytes(), 0);
        assert!(font.encode_text_with_budget("z", limits, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            font.encode_text_with_limits("abc😀中", limits).unwrap(),
            vec![0x21, 0x80, 1, 0x81, 0, 1, 0x82, 0, 0, 1]
        );
        // A short input makes table growth overlap larger than the final
        // output + eight candidate slots + two suffix slots.
        let entry = std::mem::size_of::<(u32, u8, &str)>();
        let suffix = std::mem::size_of::<Option<(u32, u8, usize)>>();
        let final_bytes = 4 + 8 * entry + 2 * suffix;
        let peak_bytes = 4 + (4 + 8) * entry;
        assert!(peak_bytes > final_bytes);
        let final_only = crate::budget::OperationBudget::new(final_bytes);
        assert!(matches!(
            font.encode_text_with_budget("a", limits, &final_only),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(final_only.live_bytes(), 0);
        let peak = crate::budget::OperationBudget::new(peak_bytes);
        let encoded = font.encode_text_with_budget("a", limits, &peak).unwrap();
        assert_eq!(encoded.as_slice(), &[0x21]);
        assert_eq!(peak.live_bytes(), encoded.capacity());
        drop(encoded);
        assert_eq!(peak.live_bytes(), 0);
        assert!(font.encode_text_with_limits("z", limits).is_err());
        assert_eq!(
            font.encode_text_with_limits("", limits).unwrap(),
            Vec::<u8>::new()
        );
        for limits in [
            crate::parser::options::Limits {
                max_objects: 4,
                ..limits
            },
            crate::parser::options::Limits {
                max_text_chars: 4,
                ..limits
            },
            crate::parser::options::Limits {
                max_decoded_bytes: 20,
                ..limits
            },
        ] {
            assert!(matches!(
                font.encode_text_with_limits("abc😀中", limits),
                Err(PdfError::ResourceLimit(_))
            ));
        }
    }

    #[test]
    fn reverse_composite_encoding_rejects_notdef_and_inconsistent_code_lengths() {
        let mut font = Type0 {
            encoding: Some(Cmap::try_new(b"1 begincodespacerange <00> <FF> endcodespacerange 1 begincidchar <21> 7 endcidchar".to_vec()).unwrap()),
            to_unicode: Some(Cmap::try_new(b"1 begincodespacerange <0000> <FFFF> endcodespacerange 1 beginbfchar <0021> <0041> endbfchar".to_vec()).unwrap()),
            ..Default::default()
        };
        let limits = crate::parser::options::Limits::default();
        assert!(font.encode_text_with_limits("A", limits).is_err());
        font.to_unicode = Some(
            Cmap::try_new(b"2 beginbfchar <21> <0041> <22> <0042> endbfchar".to_vec()).unwrap(),
        );
        assert_eq!(
            font.encode_text_with_limits("A", limits).unwrap(),
            vec![0x21]
        );
        assert!(font.encode_text_with_limits("B", limits).is_err());
        font.to_unicode = None;
        assert!(font.encode_text_with_limits("A", limits).is_err());
    }

    #[test]
    fn reverse_composite_encoding_avoids_longest_match_dead_end() {
        let font = Type0 {
            encoding: Some(Cmap::try_new(b"1 begincodespacerange <00> <FF> endcodespacerange 3 begincidchar <01> 7 <02> 8 <03> 9 endcidchar".to_vec()).unwrap()),
            to_unicode: Some(Cmap::try_new(b"3 beginbfchar <01> <0061> <02> <00610062> <03> <00620063> endbfchar".to_vec()).unwrap()),
            ..Default::default()
        };
        let limits = crate::parser::options::Limits::default();
        assert_eq!(
            font.encode_text_with_limits("abc", limits).unwrap(),
            vec![1, 3]
        );
        assert_eq!(font.encode_text_with_limits("ab", limits).unwrap(), vec![2]);
    }

    #[test]
    fn non_identity_encoding_uses_cid_metrics_but_preserves_source_unicode() {
        let syntax =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap());
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let mut parser = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            b"<< /Subtype /CIDFontType2 /BaseFont /Test /DW 100 /W [7 [700]] /DW2 [880 -1000] /W2 [7 [-1200 350 900]] >>".to_vec(),
        )).unwrap());
        let descendant = parser.get_object().unwrap();
        let mut font = Type0 {
            encoding: Some(Cmap::try_new(
                b"1 begincodespacerange <00> <FF> endcodespacerange 1 begincidchar <41> 7 endcidchar".to_vec(),
            ).unwrap()),
            to_unicode: Some(Cmap::try_new(
                b"1 beginbfchar <41> <0041> endbfchar".to_vec(),
            ).unwrap()),
            descent_font: CidFont::try_new(descendant.as_dict().unwrap(), &ctx).unwrap(),
            ..Type0::default()
        };
        let chars = font.chars(b"AB").unwrap();
        assert_eq!(chars[0].code(), 65);
        assert_eq!(chars[0].width, 700.0);
        assert_eq!(chars[1].width, 100.0);
        assert_eq!(font.text_widths(&chars).unwrap(), 800.0);
        assert_eq!(font.unicode_mapping(&chars[0]), Some("A"));
        assert!(matches!(font.get_glyph(&chars[0]), Some(GlyphDesc::Gid(7))));
        assert!(matches!(font.get_glyph(&chars[1]), Some(GlyphDesc::Gid(0))));
        font.encoding = Some(Cmap::try_new(
            b"/WMode 1 def 1 begincodespacerange <00> <FF> endcodespacerange 1 begincidchar <41> 7 endcidchar".to_vec(),
        ).unwrap());
        assert_eq!(font.chars(b"A").unwrap()[0].width, -1200.0);
        let mut visited = Vec::new();
        assert_eq!(
            font.visit_chars_limited(b"AB", 2, |character| {
                visited.push((
                    character.code(),
                    character.length(),
                    character.width(),
                    character.origin_x(),
                    character.origin_y(),
                ));
                Ok(())
            })
            .unwrap(),
            2
        );
        assert_eq!(
            visited,
            vec![(65, 1, -1200., 350., 900.), (66, 1, -1000., 50., 880.)]
        );
        let collected = font.chars_limited(b"AB", 2).unwrap();
        assert_eq!(
            collected
                .iter()
                .map(|character| (
                    character.code(),
                    character.length(),
                    character.width(),
                    character.origin_x(),
                    character.origin_y()
                ))
                .collect::<Vec<_>>(),
            visited
        );
        let mut calls = 0;
        assert!(matches!(
            font.visit_chars_limited(b"AB", 1, |_| {
                calls += 1;
                Ok(())
            }),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(calls, 1);
        calls = 0;
        assert!(matches!(
            font.visit_chars_limited(b"AB", 2, |_| {
                calls += 1;
                Err(PdfError::ObjectError("visitor stopped".into()))
            }),
            Err(PdfError::ObjectError(_))
        ));
        assert_eq!(calls, 1);
    }

    #[test]
    fn optional_unicode_mapping_distinguishes_missing_and_replacement() {
        let ch = CharCode::new(1, 1, 500.0);
        let mut font = Type0::default();
        assert_eq!(font.unicode_mapping(&ch), None);
        font.to_unicode = Some(
            Cmap::try_new_with_limits(
                b"1 beginbfchar <01> <FFFD> endbfchar".to_vec(),
                &crate::parser::options::Limits::default(),
            )
            .unwrap(),
        );
        assert_eq!(font.unicode_mapping(&ch), Some("\u{fffd}"));
        assert_eq!(font.unicode_mapping(&CharCode::new(2, 1, 500.0)), None);
    }

    #[test]
    fn malformed_type0_and_descendant_dictionaries_return_errors() {
        let syntax =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap());
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        for input in [
            "<< /BaseFont 1 >>",
            "<< /Encoding /Identity-H /DescendantFonts 1 >>",
            "<< /Encoding /Identity-H /DescendantFonts [] >>",
            "<< /Encoding /Identity-H /DescendantFonts [1] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype 1 /BaseFont /Test >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont 1 >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /DW /bad >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /DW2 [0 /bad] >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /W 1 >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /W2 1 >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test /FontDescriptor 1 >>] >>",
            "<< /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Test >>] /ToUnicode 1 >>",
        ] {
            let mut parser = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(input.as_bytes().to_vec())).unwrap(),
            );
            let object = parser.get_object().unwrap();
            assert!(
                Type0::try_new(object.as_dict().unwrap(), &ctx).is_err(),
                "{input}"
            );
        }
    }
}

impl Type0 {
    /// Reserve output capacity before encoding; internal tables are untracked.
    pub(crate) fn encode_text_with_budget(
        &self,
        text: &str,
        limits: crate::parser::options::Limits,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let count = super::pdf_font::encoding_character_count(text, limits)?;
        let capacity = count
            .checked_mul(4)
            .ok_or_else(|| PdfError::ResourceLimit("composite encoding output".into()))?;
        let lease = budget.reserve(capacity)?;
        let codes = self.encode_text_impl(text, limits, Some(budget))?;
        crate::budget::BudgetedBytes::from_reserved_vec(codes, lease)
    }

    /// Reverse existing ToUnicode through the Encoding CMap. Selects only codes
    /// with an unambiguous decoded length and nonzero CID, and verifies the final
    /// byte sequence round-trips. Does not add mappings, shape or prove glyph coverage.
    pub fn encode_text_with_limits(
        &self,
        text: &str,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<u8>> {
        self.encode_text_impl(text, limits, None)
    }

    fn encode_text_impl(
        &self,
        text: &str,
        limits: crate::parser::options::Limits,
        operation_budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Vec<u8>> {
        let count = super::pdf_font::encoding_character_count(text, limits)?;
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let encoding = self
            .encoding
            .as_ref()
            .ok_or_else(|| PdfError::FontError("composite font Encoding missing".into()))?;
        let unicode = self.to_unicode.as_ref().ok_or_else(|| {
            PdfError::UnsupportedFeature("composite Unicode encoding requires ToUnicode".into())
        })?;
        let slots = text
            .len()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("composite encoding offsets".into()))?;
        let mut budget = slots
            .checked_mul(std::mem::size_of::<Option<(u32, u8, usize)>>())
            .and_then(|size| {
                count
                    .checked_mul(4)
                    .and_then(|output| size.checked_add(output))
            })
            .filter(|size| *size <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("composite encoding working bytes".into()))?;
        let mut _mappings_lease = operation_budget.map(|pool| pool.reserve(0)).transpose()?;
        let mut mappings = Vec::new();
        let mut entries = 0usize;
        for (code, value) in unicode.unicode_entries() {
            entries += 1;
            if entries > limits.max_objects {
                return Err(PdfError::ResourceLimit("composite Unicode entries".into()));
            }
            if value.is_empty() || encoding.cid(code) == 0 {
                continue;
            }
            let bytes = code.to_be_bytes();
            for length in 1..=4u8 {
                if length < 4 && code >= (1u32 << (length as u32 * 8)) {
                    continue;
                }
                if !unicode.admits_unicode_code(code, length) {
                    continue;
                }
                let bytes = &bytes[4 - length as usize..];
                if !encoding
                    .next_char(bytes, 0)
                    .is_some_and(|ch| ch.code() == code && ch.length() == length)
                {
                    continue;
                }
                if mappings.len() >= limits.max_objects {
                    return Err(PdfError::ResourceLimit(
                        "composite encoding candidates".into(),
                    ));
                }
                budget = budget
                    .checked_add(std::mem::size_of::<(u32, u8, &str)>())
                    .filter(|size| *size <= limits.max_decoded_bytes)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("composite encoding working bytes".into())
                    })?;
                if let Some(pool) = operation_budget {
                    if mappings.len() == mappings.capacity() {
                        let capacity = mappings
                            .capacity()
                            .checked_mul(2)
                            .unwrap_or(limits.max_objects)
                            .max(1)
                            .min(limits.max_objects);
                        let entry_size = std::mem::size_of::<(u32, u8, &str)>();
                        let bytes = capacity.checked_mul(entry_size).ok_or_else(|| {
                            PdfError::ResourceLimit("composite encoding candidates".into())
                        })?;
                        // Keep the old allocation charged while copying.
                        let mut lease = pool.reserve(bytes)?;
                        let mut grown = Vec::new();
                        grown.try_reserve_exact(capacity).map_err(|_| {
                            PdfError::ResourceLimit(
                                "composite encoding candidates allocation".into(),
                            )
                        })?;
                        lease.resize(grown.capacity().checked_mul(entry_size).ok_or_else(
                            || PdfError::ResourceLimit("composite encoding candidates".into()),
                        )?)?;
                        grown.extend_from_slice(&mappings);
                        mappings = grown;
                        _mappings_lease = Some(lease);
                    }
                } else {
                    mappings.try_reserve(1).map_err(|_| {
                        PdfError::ResourceLimit("composite encoding candidates allocation".into())
                    })?;
                }
                mappings.push((code, length, value));
            }
        }
        let choice_size = std::mem::size_of::<Option<(u32, u8, usize)>>();
        let mut choices_lease = operation_budget
            .map(|pool| {
                let bytes = slots
                    .checked_mul(choice_size)
                    .ok_or_else(|| PdfError::ResourceLimit("composite encoding offsets".into()))?;
                pool.reserve(bytes)
            })
            .transpose()?;
        let mut choices = Vec::new();
        choices
            .try_reserve_exact(slots)
            .map_err(|_| PdfError::ResourceLimit("composite encoding offsets allocation".into()))?;
        if let Some(lease) = &mut choices_lease {
            lease.resize(
                choices
                    .capacity()
                    .checked_mul(choice_size)
                    .ok_or_else(|| PdfError::ResourceLimit("composite encoding offsets".into()))?,
            )?;
        }
        choices.resize(slots, None::<(u32, u8, usize)>);
        choices[text.len()] = Some((0, 0, 0));
        for (start, _) in text.char_indices().rev() {
            for (code, length, value) in &mappings {
                if text[start..].starts_with(value) {
                    let end = start + value.len();
                    if choices[end].is_some()
                        && choices[start].is_none_or(|(old, old_len, old_end)| {
                            end > old_end || (end == old_end && (*length, *code) < (old_len, old))
                        })
                    {
                        choices[start] = Some((*code, *length, end));
                    }
                }
            }
        }
        if choices[0].is_none() {
            return Err(PdfError::UnsupportedFeature(
                "text cannot be encoded by existing composite font mappings".into(),
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(
                count
                    .checked_mul(4)
                    .ok_or_else(|| PdfError::ResourceLimit("composite encoding output".into()))?,
            )
            .map_err(|_| PdfError::ResourceLimit("composite encoding output allocation".into()))?;
        let mut start = 0;
        while start < text.len() {
            let (code, length, end) = choices[start]
                .ok_or_else(|| PdfError::FontError("composite encoding suffix missing".into()))?;
            output.extend_from_slice(&code.to_be_bytes()[4 - length as usize..]);
            start = end;
        }
        let mut offset = 0;
        encoding.visit_chars_limited(&output, limits.max_text_chars, |code| {
            let value = unicode.unicode(&code).ok_or_else(|| {
                PdfError::FontError("encoded composite text lacks Unicode".into())
            })?;
            if !text[offset..].starts_with(value) {
                return Err(PdfError::FontError(
                    "composite Unicode encoding round-trip mismatch".into(),
                ));
            }
            offset += value.len();
            Ok(())
        })?;
        if offset != text.len() {
            return Err(PdfError::FontError(
                "composite Unicode encoding round-trip incomplete".into(),
            ));
        }
        Ok(output)
    }

    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        Self::try_new_inner(dict, ctx, None).map(|(font, _)| font)
    }

    pub(crate) fn try_new_with_budget<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (font, lease) = Self::try_new_inner(dict, ctx, Some(budget))?;
        Ok((font, lease.unwrap()))
    }

    fn try_new_inner<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Self, Option<crate::budget::MemoryLease>)> {
        let mut lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        let resolve = |value: &PdfObject| {
            if let Some(pool) = budget {
                ctx.resolve_owned_with_budget(value, pool)
                    .map(|(value, lease)| (value, Some(lease)))
            } else {
                ctx.resolve(value).map(|value| (value, None))
            }
        };
        let mut font = Type0::default();
        if let Some(base_font) = dict.get("BaseFont") {
            let (resolved, _temporary) = resolve(base_font)?;
            let base = resolved
                .as_name()
                .ok_or_else(|| PdfError::FontError("Type0 BaseFont must be a name".to_string()))?;
            font.base_font = Some(if let Some(pool) = budget {
                let (name, mut name_lease) = crate::budget::copy_string(base.name(), pool)?;
                lease.as_mut().unwrap().absorb(&mut name_lease)?;
                name
            } else {
                base.name().to_string()
            });
        }
        if let Some(encoding) = dict.get("Encoding") {
            let (encoding_obj, _temporary) = resolve(encoding)?;
            match encoding_obj {
                PdfObject::PdfName(enc_name) => {
                    font.encoding = Some(if let Some(pool) = budget {
                        let (cmap, mut cmap_lease) = Cmap::new_from_predefined_with_budget(
                            enc_name.name(),
                            &ctx.limits(),
                            pool,
                        )?;
                        lease.as_mut().unwrap().absorb(&mut cmap_lease)?;
                        cmap
                    } else {
                        Cmap::new_from_predefined_with_limits(enc_name.name(), &ctx.limits())?
                    });
                }
                PdfObject::PdfStream(stream) => {
                    let cmap = if let Some(pool) = budget {
                        let (cmap, mut cmap_lease) =
                            Cmap::from_stream_with_budget(&stream, ctx, pool)?;
                        lease.as_mut().unwrap().absorb(&mut cmap_lease)?;
                        cmap
                    } else {
                        Cmap::from_stream(&stream, ctx)?
                    };
                    font.encoding = Some(cmap);
                }
                _ => {
                    return Err(PdfError::FontError(format!(
                        "Type0 Encoidng need a PdfName or PdfStream got{:?}",
                        encoding_obj
                    )));
                }
            }
        } else {
            return Err(PdfError::FontError(
                "Type0 Encoding is required".to_string(),
            ));
        }

        let (descendant, _descendant_temporary) = match dict.get("DescendantFonts") {
            Some(desobj) => {
                let (des, temporary) = resolve(desobj)?;
                let array = des.into_array().ok_or_else(|| {
                    PdfError::FontError("DescendantFonts must be an array".to_string())
                })?;
                (array, temporary)
            }

            None => {
                return Err(PdfError::FontError(
                    "type0 Descdantfonts is need".to_string(),
                ));
            }
        };

        if descendant.len() != 1 {
            return Err(PdfError::FontError(
                "Type0 requires exactly one descendant font".to_string(),
            ));
        }
        let (descendant_font, _font_temporary) = resolve(
            descendant
                .get(0)
                .ok_or_else(|| PdfError::FontError("Missing descendant font".to_string()))?,
        )?;
        let descendant_dict = descendant_font.as_dict().ok_or_else(|| {
            PdfError::FontError("Descendant font must be a dictionary".to_string())
        })?;
        let cid_font = if let Some(pool) = budget {
            let (font, mut cid_lease) = CidFont::try_new_with_budget(descendant_dict, ctx, pool)?;
            lease.as_mut().unwrap().absorb(&mut cid_lease)?;
            font
        } else {
            CidFont::try_new(descendant_dict, ctx)?
        };
        font.descent_font = cid_font;
        if let Some(tu) = dict.get("ToUnicode") {
            let (tu_obj, _temporary) = resolve(tu)?;
            let stream = tu_obj
                .as_stream()
                .ok_or_else(|| PdfError::FontError("ToUnicode must be a stream".to_string()))?;
            let tu_cmap = if let Some(pool) = budget {
                let (cmap, mut cmap_lease) = Cmap::from_stream_with_budget(stream, ctx, pool)?;
                lease.as_mut().unwrap().absorb(&mut cmap_lease)?;
                cmap
            } else {
                Cmap::from_stream(stream, ctx)?
            };
            font.to_unicode = Some(tu_cmap);
        }

        Ok((font, lease))
    }

    fn char_width(&self, char: &CharCode) -> PdfResult<f32> {
        self.descent_font.char_width(&self.cid(char))
    }

    fn cid(&self, char: &CharCode) -> u32 {
        self.encoding
            .as_ref()
            .map_or(0, |cmap| cmap.cid(char.code()))
    }

    pub fn unicode(&self, ch: &CharCode) -> PdfResult<String> {
        match &self.to_unicode {
            Some(cmap) => {
                if let Some(u) = cmap.unicode(ch) {
                    return Ok(u.to_string());
                } else {
                    let mut s = String::new();
                    s.push(char::REPLACEMENT_CHARACTER);
                    return Ok(s);
                }
            }
            None => {
                let mut s = String::new();
                s.push(char::REPLACEMENT_CHARACTER);
                return Ok(s);
                //return Err(PdfError::Font("Type0 Font to_unicode is None".to_string()));
            }
        }
    }

    pub fn unicode_mapping(&self, ch: &CharCode) -> Option<&str> {
        self.to_unicode.as_ref().and_then(|cmap| cmap.unicode(ch))
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        let mut total = 0.0;
        for char in chars {
            total += self.char_width(char)?;
            if !total.is_finite() {
                return Err(PdfError::FontError("Text width overflows".to_string()));
            }
        }
        Ok(total)
    }

    pub fn writting_mode(&self) -> WritingMode {
        if let Some(cmap) = &self.encoding {
            let wmode = cmap.wmode();
            match wmode {
                Some(1) => WritingMode::Vertical,
                Some(0) => WritingMode::Horizontal,
                _ => WritingMode::Horizontal,
            }
        } else {
            WritingMode::Horizontal
        }
    }

    pub fn chars(&self, codes: &[u8]) -> PdfResult<Vec<CharCode>> {
        self.chars_limited(codes, usize::MAX)
    }

    pub fn chars_limited(&self, codes: &[u8], limit: usize) -> PdfResult<Vec<CharCode>> {
        let mut chars = Vec::new();
        self.visit_chars_limited(codes, limit, |character| {
            chars
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("font character allocation".into()))?;
            chars.push(character);
            Ok(())
        })?;
        Ok(chars)
    }

    pub(crate) fn visit_chars_limited(
        &self,
        codes: &[u8],
        limit: usize,
        mut visitor: impl FnMut(CharCode) -> PdfResult<()>,
    ) -> PdfResult<usize> {
        match &self.encoding {
            Some(cmap) => cmap.visit_chars_limited(codes, limit, |mut character| {
                match self.writting_mode() {
                    WritingMode::Vertical => {
                        if let Some(w) = self.descent_font.vertical_metrics(&self.cid(&character)) {
                            character.set_with(w.0);
                            character.set_origin_x(w.1);
                            character.set_origin_y(w.2);
                        } else {
                            println!("width is None {:?},{:?}", self.writting_mode(), character);
                        }
                    }
                    WritingMode::Horizontal => {
                        character.width = self.char_width(&character)?;
                    }
                }
                visitor(character)
            }),
            None => {
                return Err(PdfError::FontError("ENcoding is None in type0".to_string()));
            }
        }
    }
    pub fn base_font(&self) -> &str {
        match &self.base_font {
            Some(s) => s.as_str(),
            None => self.descent_font.base_font(),
        }
    }

    pub fn get_glyph(&self, code: &CharCode) -> Option<GlyphDesc> {
        self.descent_font
            .glyph_id(self.cid(code))
            .map(GlyphDesc::Gid)
    }
    pub fn fontfile(&self) -> Option<&[u8]> {
        self.descent_font.fontfile()
    }
}

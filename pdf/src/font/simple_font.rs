use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::CharCode;
use crate::font::afm::Afm;
use crate::font::cmap::Cmap;
use crate::font::descriptor::Descriptor;
use crate::font::encoding::Encoding;
use crate::objects::{PdfObject, pdf_dict::PdfDict};
use crate::pdf_context::PDFContext;

use super::GlyphDesc;
use super::afm::parse_afm_str_with_limits;
use super::builtin_font::{find_builtin_font, load_builtin_font_data, load_builtin_metrics};
use super::encoding::stand_mac_roman_name_to_unicode;
use super::font_program::{CharmapType, load_face_for_operation, use_charmap};
use super::glyph_name::adobe_glyph_list_to_unicode;

fn simple_char_code(obj: &PdfObject) -> PdfResult<u32> {
    let code = obj
        .as_number()
        .ok_or_else(|| {
            PdfError::FontError("SimpleFont character bound must be an integer".to_string())
        })?
        .as_u32_checked()
        .map_err(|_| PdfError::FontError("Invalid SimpleFont character bound".to_string()))?;
    if code > 255 {
        return Err(PdfError::FontError(
            "SimpleFont character bound exceeds 255".to_string(),
        ));
    }
    Ok(code)
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn base14_differences_without_base_keep_builtin_codes_and_overrides() {
        let document = crate::document::PdfDocument::new().unwrap();
        let mut font = PdfDict::default();
        font.try_insert("Subtype".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Type1".into()))).unwrap();
        font.try_insert("BaseFont".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Helvetica".into()))).unwrap();
        let mut encoding = PdfDict::default();
        encoding.try_insert("Differences".into(), PdfObject::PdfArray(crate::objects::pdf_array::PdfArray::from_items(vec![
            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(65.)),
            PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("B".into())),
        ]))).unwrap();
        font.try_insert("Encoding".into(), PdfObject::PdfDict(encoding)).unwrap();
        let font = SimpleFont::try_new(&font, document.ctx()).unwrap();
        assert_eq!(font.encode_text_with_limits("B C", Default::default()).unwrap(), [65,32,67]);
        assert!(font.encode_text_with_limits("A", Default::default()).is_err());
    }

    #[test]
    fn encoding_loading_budget_resolves_indirect_entries_and_retains_result() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{
                pdf_array::PdfArray, pdf_name::PdfName, pdf_number::PdfNumber,
                pdf_reference::PdfReference,
            },
        };
        let doc = Document::new().unwrap();
        let glyph = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("A".into())))
            .unwrap();
        let base = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("WinAnsiEncoding".into())))
            .unwrap();
        let array = doc
            .try_insert_object(PdfObject::PdfArray(PdfArray::from_items(vec![
                PdfObject::PdfNumber(PdfNumber::new(65.0)),
                PdfObject::PdfReference(PdfReference::from_id(glyph)),
            ])))
            .unwrap();
        let mut encoding = PdfDict::default();
        encoding.insert(
            "Differences".into(),
            PdfObject::PdfReference(PdfReference::from_id(array)),
        );
        encoding.insert(
            "BaseEncoding".into(),
            PdfObject::PdfReference(PdfReference::from_id(base)),
        );
        let encoding = doc.try_insert_object(PdfObject::PdfDict(encoding)).unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Encoding".into(),
            PdfObject::PdfReference(PdfReference::from_id(encoding)),
        );
        let probe = OperationBudget::new(65536);
        let mut font = SimpleFont::default();
        let lease = font
            .load_encoding_inner(&dict, doc.ctx(), Some(&probe))
            .unwrap()
            .unwrap();
        assert_eq!(
            font.encoding.as_ref().unwrap().unicode_mapping(65),
            Some(65)
        );
        assert_eq!(probe.live_bytes(), lease.bytes());
        assert!(lease.bytes() > 0);
        let peak = probe.peak_bytes();
        drop((font, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        let mut font = SimpleFont::default();
        assert!(
            font.load_encoding_inner(&dict, doc.ctx(), Some(&pool))
                .is_err()
        );
        assert!(font.encoding.is_none());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let lease = font
            .load_encoding_inner(&dict, doc.ctx(), Some(&pool))
            .unwrap();
        assert_eq!(
            font.encoding.as_ref().unwrap().get_glyph_name(65),
            Some("A")
        );
        drop((font, lease));
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(glyph, PdfObject::PdfNull).unwrap();
        assert!(
            SimpleFont::default()
                .load_encoding_inner(&dict, doc.ctx(), Some(&pool))
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn width_loading_budget_tracks_indirect_array_and_preserves_state_on_error() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{pdf_array::PdfArray, pdf_number::PdfNumber, pdf_reference::PdfReference},
        };
        let doc = Document::new().unwrap();
        let number = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(500.0)))
            .unwrap();
        let values = PdfArray::from_items(vec![
            PdfObject::PdfReference(PdfReference::from_id(number)),
            PdfObject::PdfNumber(PdfNumber::new(600.0)),
        ]);
        let array = doc.try_insert_object(PdfObject::PdfArray(values)).unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Widths".into(),
            PdfObject::PdfReference(PdfReference::from_id(array)),
        );
        let mut font = SimpleFont {
            first_char: Some(0),
            last_char: Some(1),
            ..Default::default()
        };
        let probe = OperationBudget::new(65536);
        font.load_width_inner(&dict, doc.ctx(), Some(&probe))
            .unwrap();
        assert_eq!(&font.widths.unwrap()[..2], &[500.0, 600.0]);
        let peak = probe.peak_bytes();
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        let previous = font.widths;
        assert!(
            font.load_width_inner(&dict, doc.ctx(), Some(&pool))
                .is_err()
        );
        assert_eq!(font.widths, previous);
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        font.load_width_inner(&dict, doc.ctx(), Some(&pool))
            .unwrap();
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(number, PdfObject::PdfNull).unwrap();
        assert!(
            font.load_width_inner(&dict, doc.ctx(), Some(&probe))
                .is_err()
        );
        assert_eq!(font.widths, previous);
        assert_eq!(probe.live_bytes(), 0);
    }

    #[test]
    fn fixed_glyph_table_preserves_all_byte_codes_zero_and_clone_isolation() {
        let mut table = SimpleGlyphMap::default();
        for code in 0..=255u8 {
            assert_eq!(table.get(&code), None);
            table.insert(code, u32::from(code));
        }
        let mut copy = table.clone();
        copy.insert(255, u32::MAX);
        for code in 0..=255u8 {
            assert_eq!(table.get(&code), Some(&u32::from(code)));
        }
        assert_eq!(copy.get(&0), Some(&0));
        assert_eq!(copy.get(&255), Some(&u32::MAX));
        assert_eq!(table.get(&255), Some(&255));
    }
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::io::Cursor;

    #[test]
    fn reverse_encoding_fixed_table_accepts_all_256_candidates() {
        let mut source =
            String::from("1 begincodespacerange <00> <FF> endcodespacerange 256 beginbfchar ");
        for code in 0..=255u16 {
            source.push_str(&format!("<{code:02X}> <{code:04X}> "));
        }
        source.push_str("endbfchar");
        let font = SimpleFont {
            to_unicode: Some(Cmap::try_new(source.into_bytes()).unwrap()),
            ..Default::default()
        };
        let limits = crate::parser::options::Limits {
            max_objects: 256,
            ..Default::default()
        };
        let text: String = (0..=255u32)
            .map(|code| char::from_u32(code).unwrap())
            .collect();
        let expected: Vec<u8> = (0..=255u8).collect();
        assert_eq!(
            font.encode_text_with_limits(&text, limits).unwrap(),
            expected
        );
        let budget = crate::budget::OperationBudget::new(64 * 1024);
        let encoded = font
            .encode_text_with_budget(&text, limits, &budget)
            .unwrap();
        assert_eq!(encoded.as_slice(), expected);
        assert_eq!(budget.live_bytes(), encoded.capacity());
        drop(encoded);
        assert_eq!(budget.live_bytes(), 0);
        let limited = crate::parser::options::Limits {
            max_objects: 255,
            ..limits
        };
        assert!(matches!(
            font.encode_text_with_budget("ÿ", limited, &budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            font.encode_text_with_budget("ÿ", limits, &budget)
                .unwrap()
                .as_slice(),
            &[255]
        );
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn reverse_encoding_uses_actual_mappings_and_encodable_suffixes() {
        let cmap = Cmap::try_new(b"1 begincodespacerange <00> <FF> endcodespacerange 5 beginbfchar <01> <00610062> <02> <0061> <03> <00620063> <04> <0061> <05> <D83DDE00> endbfchar".to_vec()).unwrap();
        let font = SimpleFont {
            to_unicode: Some(cmap),
            ..Default::default()
        };
        let limits = crate::parser::options::Limits::default();
        let maximum = 3 + 4 * std::mem::size_of::<Option<(u8, usize)>>();
        let budget = crate::budget::OperationBudget::new(maximum);
        let codes = font
            .encode_text_with_budget("abc", limits, &budget)
            .unwrap();
        assert_eq!(codes.as_slice(), &[2, 3]);
        assert_eq!(budget.live_bytes(), codes.capacity());
        let neighbor = budget.reserve(maximum - budget.live_bytes()).unwrap();
        assert!(font.encode_text_with_budget("a", limits, &budget).is_err());
        drop(neighbor);
        drop(codes);
        assert_eq!(budget.live_bytes(), 0);
        assert!(font.encode_text_with_budget("z", limits, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            font.encode_text_with_limits("abc", limits).unwrap(),
            vec![2, 3]
        );
        assert_eq!(font.encode_text_with_limits("ab", limits).unwrap(), vec![1]);
        assert_eq!(
            font.encode_text_with_limits("a😀", limits).unwrap(),
            vec![2, 5]
        );
        assert_eq!(
            font.encode_text_with_limits("", limits).unwrap(),
            Vec::<u8>::new()
        );
        assert!(font.encode_text_with_limits("z", limits).is_err());
        assert!(font.encode_text_with_limits("\u{fffd}", limits).is_err());
        for limits in [
            crate::parser::options::Limits {
                max_text_chars: 2,
                ..limits
            },
            crate::parser::options::Limits {
                max_objects: 4,
                ..limits
            },
            crate::parser::options::Limits {
                max_decoded_bytes: 10,
                ..limits
            },
        ] {
            assert!(matches!(
                font.encode_text_with_limits("abc", limits),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert_eq!(
            font.encode_text_with_limits("abc", crate::parser::options::Limits::default())
                .unwrap(),
            vec![2, 3]
        );
    }

    #[test]
    fn reverse_encoding_falls_back_only_when_tounicode_is_absent() {
        let limits = crate::parser::options::Limits::default();
        let mut font = SimpleFont {
            encoding: Some(Encoding::new_from_name("WinAnsiEncoding").unwrap()),
            ..Default::default()
        };
        assert_eq!(
            font.encode_text_with_limits("A€", limits).unwrap(),
            vec![65, 128]
        );
        assert!(font.encode_text_with_limits("中文", limits).is_err());
        font.to_unicode = Some(Cmap::default());
        assert!(font.encode_text_with_limits("A", limits).is_err());
        font.to_unicode = Some(Cmap::try_new(b"1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <01> <0041> endbfchar".to_vec()).unwrap());
        assert_eq!(font.encode_text_with_limits("A", limits).unwrap(), vec![1]);
    }

    #[test]
    fn builtin_metrics_obey_document_limits() {
        use crate::parser::options::{Limits, ParseOptions};
        let input_len = load_builtin_metrics("Courier").unwrap().len();
        for (bytes, entries, accepted) in [
            (input_len, 315, true),
            (input_len - 1, 315, false),
            (input_len, 314, false),
        ] {
            let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
                PdfDict::default(),
                ParseOptions {
                    limits: Limits {
                        max_decoded_bytes: bytes,
                        max_objects: entries,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            let mut font = SimpleFont {
                base_font: "Courier".into(),
                ..Default::default()
            };
            let result = font.load_width(&PdfDict::default(), &ctx);
            if accepted {
                result.unwrap();
                assert_eq!(font.afm.as_ref().unwrap().get_char_width("A"), Some(600.0));
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
                assert!(font.afm.is_none());
            }
        }
    }

    #[test]
    fn partial_widths_use_descriptor_missing_width_without_replacing_explicit_zero() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            Default::default(),
        );
        for subtype in ["Type1", "TrueType"] {
            let text = format!(
                "<< /Type /Font /Subtype /{subtype} /BaseFont /Courier \
                 /Encoding /WinAnsiEncoding /FirstChar 32 /LastChar 33 /Widths [500 0] \
                 /FontDescriptor << /Type /FontDescriptor /FontName /Courier \
                 /Flags 33 /FontBBox [0 -200 600 800] /ItalicAngle 0 \
                 /Ascent 800 /Descent -200 /CapHeight 600 /StemV 80 /MissingWidth 600 >> >>"
            );
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(text.into_bytes())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let ordinary = SimpleFont::try_new(&dict, &ctx).unwrap();
            let pool = crate::budget::OperationBudget::new(16 * 1024 * 1024);
            let (budgeted, lease) = SimpleFont::try_new_with_budget(&dict, &ctx, &pool).unwrap();
            for font in [&ordinary, &budgeted] {
                assert_eq!(font.char_width(32).unwrap(), 500.0);
                assert_eq!(font.char_width(33).unwrap(), 0.0);
                for code in [0, 31, 34, b'A', b'T', 255] {
                    assert_eq!(font.char_width(code).unwrap(), 600.0);
                }
            }
            drop(budgeted);
            drop(lease);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn width_arrays_and_unicode_are_checked() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let dict = |text: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap()
        };
        for (first, last, data) in [
            (None, Some(1), "[500]"),
            (Some(1), None, "[500]"),
            (Some(2), Some(1), "[]"),
            (Some(255), Some(256), "[500 600]"),
            (Some(0), Some(1), "[500]"),
            (Some(0), Some(0), "[/bad]"),
            (Some(0), Some(0), "1"),
        ] {
            let mut font = SimpleFont {
                first_char: first,
                last_char: last,
                ..Default::default()
            };
            assert!(
                font.load_width(&dict(&format!("<< /Widths {data} >>")), &ctx)
                    .is_err()
            );
        }
        let mut font = SimpleFont {
            first_char: Some(254),
            last_char: Some(255),
            ..Default::default()
        };
        font.load_width(&dict("<< /Widths [500 600] >>"), &ctx)
            .unwrap();
        assert_eq!(font.char_width(255).unwrap(), 600.0);
        assert_eq!(font.char_width(65).unwrap(), 0.0);
        font.to_unicode = Some(Cmap::default());
        assert_eq!(
            font.unicode_mapping(&CharCode::new(1, 1, 0.0)).unwrap(),
            None
        );
        assert!(font.unicode_mapping(&CharCode::new(256, 2, 0.0)).is_err());
        assert_eq!(font.unicode(&CharCode::new(1, 1, 0.0)).unwrap(), "\u{fffd}");
        assert!(font.unicode(&CharCode::new(256, 2, 0.0)).is_err());
        font.to_unicode = None;
        font.encoding = Some(Encoding::new_from_name("WinAnsiEncoding").unwrap());
        assert_eq!(
            font.unicode_mapping(&CharCode::new(65, 1, 0.0))
                .unwrap()
                .as_deref(),
            Some("A")
        );
        assert_eq!(
            font.unicode_mapping(&CharCode::new(0, 1, 0.0)).unwrap(),
            None
        );
        assert!(font.text_widths(&[CharCode::new(256, 2, 0.0)]).is_err());
        for code in ["-1", "1.5", "256", "/bad"] {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(format!("[{code}]").into_bytes())).unwrap(),
            )
            .get_object()
            .unwrap();
            assert!(simple_char_code(obj.as_array().unwrap().get(0).unwrap()).is_err());
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SimpleSubType {
    TrueType,
    #[default]
    Type1,
    MMType1,
}

#[derive(Debug, Default, Clone)]
pub struct SimpleFont {
    base_font: String,
    to_unicode: Option<Cmap>,
    descriptor: Descriptor,
    first_char: Option<u32>,
    last_char: Option<u32>,
    widths: Option<[f32; 256]>,
    encoding: Option<Encoding>,
    is_embed: bool,
    subtype: SimpleSubType,
    code_to_gid: SimpleGlyphMap,
    afm: Option<Afm>,
}

/// Simple fonts have exactly 256 possible character codes. Keep the mapping
/// inline, distinguishing an absent mapping from the valid .notdef glyph zero.
#[derive(Debug, Clone)]
struct SimpleGlyphMap([Option<u32>; 256]);

impl Default for SimpleGlyphMap {
    fn default() -> Self {
        Self([None; 256])
    }
}

impl SimpleGlyphMap {
    fn insert(&mut self, code: u8, glyph: u32) {
        self.0[usize::from(code)] = Some(glyph);
    }
    fn get(&self, code: &u8) -> Option<&u32> {
        self.0[usize::from(*code)].as_ref()
    }
}

#[derive(Clone, Copy)]
enum EncodingMapping<'a> {
    Borrowed(&'a str),
    Scalar(char),
}

impl EncodingMapping<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Borrowed(value) => value.len(),
            Self::Scalar(value) => value.len_utf8(),
        }
    }
    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Borrowed(value) => text.starts_with(value),
            Self::Scalar(value) => text.starts_with(*value),
        }
    }
}

impl SimpleFont {
    /// Reserve the existing encoder's output capacity before encoding starts.
    /// Internal mappings are not yet charged to this pool.
    pub(crate) fn encode_text_with_budget(
        &self,
        text: &str,
        limits: crate::parser::options::Limits,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let count = super::pdf_font::encoding_character_count(text, limits)?;
        let lease = budget.reserve(count)?;
        let codes = self.encode_text_impl(text, limits, Some(budget))?;
        crate::budget::BudgetedBytes::from_reserved_vec(codes, lease)
    }

    /// Encode through this font's existing Unicode mappings, not a generic
    /// WinAnsi table. Supports multi-scalar mappings; never inserts replacement
    /// codes or updates ToUnicode. This is not shaping or glyph-coverage proof.
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
        let slots = text
            .len()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("font encoding offsets".into()))?;
        let mut budget = slots
            .checked_mul(std::mem::size_of::<Option<(u8, usize)>>())
            .and_then(|size| size.checked_add(count))
            .ok_or_else(|| PdfError::ResourceLimit("font encoding working bytes".into()))?;
        if budget > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "font encoding working bytes".into(),
            ));
        }
        // Simple fonts have exactly 256 possible code values. Keep borrowed
        // candidates in a bounded stack table, without a growing heap Vec.
        // This stack frame is not charged to the shared allocation budget.
        let mut mappings = [None; 256];
        let mut mapping_count = 0;
        for code in 0..=255u8 {
            let unicode = if let Some(cmap) = &self.to_unicode {
                cmap.unicode(&CharCode::new(code as u32, 1, 0.0))
                    .map(EncodingMapping::Borrowed)
            } else {
                self.encoding
                    .as_ref()
                    .and_then(|encoding| encoding.unicode_mapping(code))
                    .and_then(char::from_u32)
                    .map(EncodingMapping::Scalar)
            };
            let Some(unicode) = unicode else {
                continue;
            };
            if unicode.len() == 0 {
                continue;
            }
            if mapping_count >= limits.max_objects {
                return Err(PdfError::ResourceLimit("font encoding mappings".into()));
            }
            budget = budget
                .checked_add(std::mem::size_of::<(u8, EncodingMapping<'_>)>())
                .filter(|size| *size <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("font encoding working bytes".into()))?;
            mappings[mapping_count] = Some((code, unicode));
            mapping_count += 1;
        }
        let choice_size = std::mem::size_of::<Option<(u8, usize)>>();
        let mut choices_lease = operation_budget
            .map(|pool| {
                pool.reserve(
                    slots
                        .checked_mul(choice_size)
                        .ok_or_else(|| PdfError::ResourceLimit("font encoding offsets".into()))?,
                )
            })
            .transpose()?;
        let mut choices = Vec::new();
        choices
            .try_reserve_exact(slots)
            .map_err(|_| PdfError::ResourceLimit("font encoding offsets allocation".into()))?;
        if let Some(lease) = &mut choices_lease {
            lease.resize(
                choices
                    .capacity()
                    .checked_mul(choice_size)
                    .ok_or_else(|| PdfError::ResourceLimit("font encoding offsets".into()))?,
            )?;
        }
        choices.resize(slots, None::<(u8, usize)>);
        choices[text.len()] = Some((0, 0));
        // Work backwards so a longest mapping is chosen only if its suffix
        // remains encodable. Equal mappings deterministically use the lower code.
        for (start, _) in text.char_indices().rev() {
            for (code, unicode) in mappings[..mapping_count].iter().flatten() {
                if unicode.matches(&text[start..]) {
                    let end = start + unicode.len();
                    if choices[end].is_some()
                        && choices[start].is_none_or(|(old, old_end)| {
                            end > old_end || (end == old_end && *code < old)
                        })
                    {
                        choices[start] = Some((*code, end));
                    }
                }
            }
        }
        if choices[0].is_none() {
            return Err(PdfError::UnsupportedFeature(
                "text cannot be encoded by the font's existing Unicode mappings".into(),
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("font encoding output allocation".into()))?;
        let mut start = 0;
        while start < text.len() {
            let (code, end) = choices[start]
                .ok_or_else(|| PdfError::FontError("font encoding suffix missing".into()))?;
            output.push(code);
            start = end;
        }
        Ok(output)
    }

    pub fn base_font(&self) -> &str {
        self.base_font.as_str()
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
        let resolve =
            |object: &PdfObject| -> PdfResult<(PdfObject, Option<crate::budget::MemoryLease>)> {
                if let Some(pool) = budget {
                    let (object, lease) = ctx.resolve_owned_with_budget(object, pool)?;
                    Ok((object, Some(lease)))
                } else {
                    Ok((ctx.resolve(object)?, None))
                }
            };
        let subtype = match dict.get("Subtype") {
            Some(st) => {
                let (resolved, _temporary) = resolve(st)?;
                let tn = resolved
                    .as_name()
                    .ok_or_else(|| {
                        PdfError::FontError("SimpleFont Subtype must be a name".to_string())
                    })?
                    .name();
                match tn {
                    "TrueType" => SimpleSubType::TrueType,
                    "Type1" => SimpleSubType::Type1,
                    "MMType1" => SimpleSubType::MMType1,
                    _ => {
                        return Err(PdfError::FontError(
                            "Simple font subtype  is invalid".to_string(),
                        ));
                    }
                }
            }
            None => {
                return Err(PdfError::FontError(
                    "Simple font subtype is None".to_string(),
                ));
            }
        };
        let mut font = SimpleFont {
            subtype,
            is_embed: false,
            ..Default::default()
        };

        if let Some(bf) = dict.get("BaseFont") {
            let (resolved, _temporary) = resolve(bf)?;
            let name = resolved.as_name().ok_or_else(|| {
                PdfError::FontError("SimpleFont BaseFont must be a name".to_string())
            })?;
            font.base_font = if let Some(pool) = budget {
                let (name, mut name_lease) = crate::budget::copy_string(name.name(), pool)?;
                lease.as_mut().unwrap().absorb(&mut name_lease)?;
                name
            } else {
                name.name().to_string()
            };
        }

        if let Some(first_char) = dict.get("FirstChar") {
            let (resolved, _temporary) = resolve(first_char)?;
            font.first_char = Some(simple_char_code(&resolved)?);
        }

        if let Some(last_char) = dict.get("LastChar") {
            let (resolved, _temporary) = resolve(last_char)?;
            font.last_char = Some(simple_char_code(&resolved)?);
        }
        if let Some(mut descriptor_lease) = font.load_descriptor_inner(dict, ctx, budget)? {
            lease.as_mut().unwrap().absorb(&mut descriptor_lease)?;
        }
        if let Some(mut encoding_lease) = font.load_encoding_inner(dict, ctx, budget)? {
            lease.as_mut().unwrap().absorb(&mut encoding_lease)?;
        }
        if let Some(mut unicode_lease) = font.load_to_unicode_inner(dict, ctx, budget)? {
            lease.as_mut().unwrap().absorb(&mut unicode_lease)?;
        }
        if let Some(mut afm_lease) = font.load_width_inner(dict, ctx, budget)? {
            lease.as_mut().unwrap().absorb(&mut afm_lease)?;
        }
        font.init_cid_to_gid(budget)?;

        // TODO code to gid

        Ok((font, lease))
    }
    fn code_to_name(&self, code: u8) -> Option<&str> {
        match &self.encoding {
            Some(enc) => enc.get_glyph_name(code),
            None => None,
        }
    }

    fn init_truetype_cid_to_gid(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<()> {
        if let Some(fontfile) = self.fontfile() {
            let face = load_face_for_operation(fontfile, budget)?;
            let charmap_type = use_charmap(self.descriptor.is_symbolic(), &face)?;

            for charcode in 0..=255 {
                if let Some(gname) = self.code_to_name(charcode) {
                    match charmap_type {
                        CharmapType::MsSymbol => {
                            let idx = charcode as u16 + 0xf00;
                            let gid = face.get_char_index(idx as usize).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                        CharmapType::MacRoman => {
                            let unicode = stand_mac_roman_name_to_unicode(charcode).unwrap_or(0);
                            let gid = face.get_char_index(unicode as usize).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                        CharmapType::MsUnicode => {
                            let unicode = adobe_glyph_list_to_unicode(gname).unwrap_or(0);
                            let gid = face.get_char_index(unicode as usize).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                        CharmapType::Other => {
                            let gid = face.get_name_index(gname).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                    }
                } else {
                    match charmap_type {
                        CharmapType::MsSymbol => {
                            let gid = face.get_char_index(0xF000 + charcode as usize).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                        _ => {
                            let gid = face.get_char_index(charcode as usize).unwrap_or(0);
                            self.code_to_gid.insert(charcode, gid);
                        }
                    }
                }
            }
        } else {
            println!("fontfile is none");
        }
        Ok(())
    }

    fn init_type1_cid_to_gid(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<()> {
        if let Some(fontfile) = self.fontfile() {
            let face = load_face_for_operation(fontfile, budget)?;
            for charcode in 0..=255 {
                if let Some(enc) = &self.encoding {
                    if let Some(name) = enc.get_glyph_name(charcode) {
                        let gid = face.get_name_index(name).unwrap_or(0);
                        self.code_to_gid.insert(charcode, gid);
                    } else {
                        let gid = face.get_char_index(charcode as usize).unwrap_or(0);
                        self.code_to_gid.insert(charcode, gid);
                    }
                } else {
                    let gid = face.get_char_index(charcode as usize).unwrap_or(0);
                    self.code_to_gid.insert(charcode, gid);
                }
            }
        }
        Ok(())
    }

    fn init_cid_to_gid(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<()> {
        match self.subtype {
            SimpleSubType::Type1 | SimpleSubType::MMType1 => self.init_type1_cid_to_gid(budget),
            SimpleSubType::TrueType => self.init_truetype_cid_to_gid(budget),
        }
    }

    pub fn chars(&self, codes: &[u8]) -> PdfResult<Vec<CharCode>> {
        self.chars_limited(codes, usize::MAX)
    }

    pub fn chars_limited(&self, codes: &[u8], limit: usize) -> PdfResult<Vec<CharCode>> {
        if codes.len() > limit {
            return Err(PdfError::ResourceLimit("decoded font characters".into()));
        }
        let mut res = Vec::new();
        res.try_reserve_exact(codes.len())
            .map_err(|_| PdfError::ResourceLimit("font character allocation".into()))?;
        self.visit_chars_limited(codes, limit, |character| {
            res.push(character);
            Ok(())
        })?;
        Ok(res)
    }

    pub(crate) fn visit_chars_limited(
        &self,
        codes: &[u8],
        limit: usize,
        mut visitor: impl FnMut(CharCode) -> PdfResult<()>,
    ) -> PdfResult<usize> {
        if codes.len() > limit {
            return Err(PdfError::ResourceLimit("decoded font characters".into()));
        }
        for &code in codes {
            visitor(CharCode::new(u32::from(code), 1, self.char_width(code)?))?;
        }
        Ok(codes.len())
    }

    fn load_to_unicode_inner<R: Seek + Read>(
        &mut self,
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<crate::budget::MemoryLease>> {
        let mut result_lease = None;
        if let Some(tu) = dict.get("ToUnicode") {
            let (tuobj, _temporary) = if let Some(pool) = budget {
                let (object, lease) = ctx.resolve_owned_with_budget(tu, pool)?;
                (object, Some(lease))
            } else {
                (ctx.resolve(tu)?, None)
            };
            let stream = tuobj
                .as_stream()
                .ok_or_else(|| PdfError::FontError("ToUnicode must be a stream".to_string()))?;
            let cmap = if let Some(pool) = budget {
                let (cmap, lease) = Cmap::from_stream_with_budget(stream, ctx, pool)?;
                result_lease = Some(lease);
                cmap
            } else {
                Cmap::from_stream(stream, ctx)?
            };
            self.to_unicode = Some(cmap);
        }
        Ok(result_lease)
    }

    fn load_width<R: Seek + Read>(&mut self, dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<()> {
        self.load_width_inner(dict, ctx, None).map(|_| ())
    }

    fn load_width_inner<R: Seek + Read>(
        &mut self,
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<crate::budget::MemoryLease>> {
        let resolve =
            |object: &PdfObject| -> PdfResult<(PdfObject, Option<crate::budget::MemoryLease>)> {
                if let Some(pool) = budget {
                    let (object, lease) = ctx.resolve_owned_with_budget(object, pool)?;
                    Ok((object, Some(lease)))
                } else {
                    Ok((ctx.resolve(object)?, None))
                }
            };
        if let Some(ws) = dict.get("Widths") {
            let (ws, _array_lease) = resolve(ws)?;
            let wsa = ws
                .as_array()
                .ok_or_else(|| PdfError::FontError("Widths must be an array".to_string()))?;
            let first = self
                .first_char
                .ok_or_else(|| PdfError::FontError("Widths requires FirstChar".to_string()))?;
            let last = self
                .last_char
                .ok_or_else(|| PdfError::FontError("Widths requires LastChar".to_string()))?;
            if first > last || last > 255 || wsa.len() != (last - first + 1) as usize {
                return Err(PdfError::FontError(
                    "Widths length does not match FirstChar/LastChar".to_string(),
                ));
            }
            // Codes outside FirstChar..=LastChar use MissingWidth (default 0),
            // not an explicit zero advance. Producers may provide only space's
            // width for a fixed-pitch signature appearance such as Courier.
            let mut widths = [self.descriptor.missing_width(); 256];
            for (i, v) in wsa.into_iter().enumerate() {
                let index = i + first as usize;
                let (number, _number_lease) = resolve(v)?;
                widths[index] = number.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                    PdfError::FontError("Widths must contain finite numbers".to_string())
                })?;
            }
            self.widths = Some(widths);
        }
        if let Some(name) = find_builtin_font(self.base_font()) {
            if let Some(afm_data) = load_builtin_metrics(name) {
                if afm_data.len() > ctx.limits().max_decoded_bytes {
                    return Err(PdfError::ResourceLimit("AFM input bytes".into()));
                }
                let s = std::str::from_utf8(afm_data)
                    .map_err(|_| PdfError::FontError("afm data not utf8".to_string()))?;
                let (afm, lease) = if let Some(pool) = budget {
                    let (afm, lease) =
                        super::afm::parse_afm_str_with_budget(s, &ctx.limits(), pool)?;
                    (afm, Some(lease))
                } else {
                    (parse_afm_str_with_limits(s, &ctx.limits())?, None)
                };
                self.afm = Some(afm);
                return Ok(lease);
            }
        }
        Ok(None)
    }

    fn load_descriptor_inner<R: Seek + Read>(
        &mut self,
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<crate::budget::MemoryLease>> {
        let mut result_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        if let Some(desc) = dict.get("FontDescriptor") {
            let (resolved, _temporary) = if let Some(pool) = budget {
                let (object, lease) = ctx.resolve_owned_with_budget(desc, pool)?;
                (object, Some(lease))
            } else {
                (ctx.resolve(desc)?, None)
            };
            let desc_dict = resolved.as_dict().ok_or_else(|| {
                PdfError::FontError("FontDescriptor must be a dictionary".to_string())
            })?;
            let desc = if let Some(pool) = budget {
                let (descriptor, mut lease) =
                    Descriptor::try_new_with_budget(desc_dict, ctx, pool)?;
                result_lease.as_mut().unwrap().absorb(&mut lease)?;
                descriptor
            } else {
                Descriptor::try_new(desc_dict, ctx)?
            };
            if desc.fontfile().is_some() {
                self.is_embed = true;
            }
            self.descriptor = desc;
        } else {
            self.is_embed = true;
        }
        Ok(result_lease)
    }

    fn load_encoding_inner<R: Seek + Read>(
        &mut self,
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<crate::budget::MemoryLease>> {
        let mut result_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        if let Some(enc) = dict.get("Encoding") {
            let (enc, _temporary) = if let Some(pool) = budget {
                let (object, lease) = ctx.resolve_owned_with_budget(enc, pool)?;
                (object, Some(lease))
            } else {
                (ctx.resolve(enc)?, None)
            };
            match enc {
                PdfObject::PdfDict(d) => {
                    let encoding = if let Some(pool) = budget {
                        let (encoding, mut lease) =
                            Encoding::try_new_with_context_and_budget(&d, ctx, pool)?;
                        result_lease.as_mut().unwrap().absorb(&mut lease)?;
                        encoding
                    } else {
                        Encoding::try_new_with_context(&d, ctx)?
                    };
                    self.encoding = Some(encoding);
                }
                PdfObject::PdfName(name) => {
                    let encoding = Encoding::new_from_name(name.name())?;
                    self.encoding = Some(encoding);
                }
                _ => {
                    return Err(PdfError::FontError(
                        "Simple Font Encoding need Name Dict or Indirect".to_string(),
                    ));
                }
            }
        }
        if !self.descriptor.is_embed() && matches!(self.subtype, SimpleSubType::Type1) {
            if let (Some(encoding), Some(builtin)) = (&mut self.encoding, find_builtin_font(self.base_font.as_str()).filter(|name| *name == self.base_font.as_str())) {
                let base = match builtin { "Symbol" => "Symbol", "ZapfDingbats" => "ZapfDingbats", _ => "StandardEncoding" };
                encoding.default_base(base)?;
            }
        }

        // TODO set default encoding
        if self.encoding.is_none() {
            if let Some(builtin_name) = find_builtin_font(self.base_font.as_str()) {
                if builtin_name == "Symbol" {
                    let encoding = Encoding::new_from_name("Symbol")?;
                    self.encoding = Some(encoding);
                } else if builtin_name == "ZapfDingbats" {
                    let encoding = Encoding::new_from_name("ZapfDingbats")?;
                    self.encoding = Some(encoding);
                } else {
                    let encoding = Encoding::new_from_name("StandardEncoding")?;
                    self.encoding = Some(encoding);
                }
            }
        }
        Ok(result_lease)
    }

    pub fn char_width(&self, code: u8) -> PdfResult<f32> {
        if let Some(widths) = self.widths {
            return Ok(widths[code.to_owned() as usize]);
        }
        if let Some(afm) = self.afm.as_ref() {
            if let Some(enc) = &self.encoding {
                if let Some(name) = enc.get_glyph_name(code) {
                    if let Some(w) = afm.get_char_width(name) {
                        return Ok(w);
                    }
                }
            }
        }
        return Err(PdfError::FontError("Font width is erro".to_string()));
    }

    pub fn unicode_mapping(&self, ch: &CharCode) -> PdfResult<Option<String>> {
        if ch.code() > 255 {
            return Err(PdfError::FontError(
                "SimpleFont character code exceeds 255".into(),
            ));
        }
        if let Some(cmap) = &self.to_unicode {
            return Ok(cmap.unicode(ch).map(str::to_owned));
        }
        Ok(self
            .encoding
            .as_ref()
            .and_then(|encoding| encoding.unicode_mapping(ch.code() as u8))
            .and_then(char::from_u32)
            .map(|value| value.to_string()))
    }

    pub(crate) fn matches_unicode(&self, ch: &CharCode, expected: &str) -> bool {
        if ch.length() != 1 || ch.code() > 255 { return false; }
        if let Some(cmap) = &self.to_unicode { return cmap.unicode(ch) == Some(expected); }
        let actual = self.encoding.as_ref()
            .and_then(|encoding| encoding.unicode_mapping(ch.code() as u8))
            .and_then(char::from_u32);
        let mut chars = expected.chars();
        actual.is_some() && actual == chars.next() && chars.next().is_none()
    }

    pub fn unicode(&self, ch: &CharCode) -> PdfResult<String> {
        if ch.code() > 255 {
            return Err(PdfError::FontError(
                "SimpleFont character code exceeds 255".to_string(),
            ));
        }
        // TODO handle simple font to_unicode is None
        match &self.to_unicode {
            Some(cmap) => {
                let u = cmap.unicode(ch).unwrap_or("\u{fffd}");
                Ok(u.to_string())
            }
            None => match &self.encoding {
                Some(enc) => {
                    let u = enc.unicode_from_charcode(&(ch.code() as u8));
                    let c = char::from_u32(u).unwrap_or(char::REPLACEMENT_CHARACTER);
                    let mut s = String::new();
                    s.push(c);
                    Ok(s)
                }
                None => {
                    //let mut fnf = std::fs::File::create("truetype_without_encoding.ttf").unwrap();
                    Err(PdfError::FontError(
                        "Simple Font to_unicode is None".to_string(),
                    ))
                }
            },
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        let mut total_width: f32 = 0.0;
        for c in chars {
            if c.code() > 255 {
                return Err(PdfError::FontError(
                    "SimpleFont character code exceeds 255".to_string(),
                ));
            }
            let w = self.char_width(c.code() as u8)?;
            total_width += w;
            if !total_width.is_finite() {
                return Err(PdfError::FontError("Text width overflows".to_string()));
            }
        }
        Ok(total_width)
    }

    pub fn get_glyph(&self, code: &CharCode) -> Option<GlyphDesc> {
        let c = code.code() as u8;
        if let Some(gid) = self.code_to_gid.get(&c) {
            return Some(GlyphDesc::Gid(gid.to_owned()));
        }
        None

        // encoding 获取 glypn_name ,用 name 去查 glyph
    }

    pub fn fontfile(&self) -> Option<&[u8]> {
        if let Some(o) = self.descriptor.fontfile() {
            Some(o)
        } else {
            load_builtin_font_data(&self.base_font)
        }
    }

    pub fn has_embedded_program(&self) -> bool {
        self.descriptor.fontfile().is_some()
    }
}

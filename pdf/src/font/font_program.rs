use crate::error::{PdfError, PdfResult};

use super::{
    open_type_program::OpenTypeProgram, truetype::true_type_program::TrueTypeProgram,
    type1_program::Type1Program,
};
use freetype::{Face, Library};

#[derive(Debug, Clone)]
pub enum FontProgram {
    Type1(Type1Program),
    TrueType(TrueTypeProgram),
    OpenType(OpenTypeProgram),
}

pub fn load_freetype_face(fontfile: Vec<u8>) -> PdfResult<Face> {
    let lib = Library::init().map_err(|e| {
        if e == freetype::Error::OutOfMemory {
            PdfError::ResourceLimit("FreeType library allocation".into())
        } else {
            PdfError::FontError(format!("Freetype library init error:{:?}", e))
        }
    })?;
    match lib.new_memory_face(fontfile, 0) {
        Ok(face) => Ok(face),
        Err(freetype::Error::OutOfMemory) => {
            Err(PdfError::ResourceLimit("FreeType face allocation".into()))
        }
        Err(e) => Err(PdfError::FontError(format!(
            "Load Freetype face error{:?}",
            e
        ))),
    }
}

/// Owns the Rust input-copy quota until FreeType releases its memory face.
/// FreeType's native allocations are not charged to this pool.
pub(crate) struct OperationFace {
    face: Face,
    _lease: Option<crate::budget::MemoryLease>,
}

impl std::ops::Deref for OperationFace {
    type Target = Face;
    fn deref(&self) -> &Self::Target {
        &self.face
    }
}

pub(crate) fn load_face_for_operation(
    fontfile: &[u8],
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<OperationFace> {
    let (data, lease) = if let Some(pool) = budget {
        let (data, lease) =
            crate::budget::BudgetedBytes::copy_from_slice(pool, fontfile)?.into_reserved_parts();
        (data, Some(lease))
    } else {
        (fontfile.to_vec(), None)
    };
    let face = load_freetype_face(data)?;
    Ok(OperationFace {
        face,
        _lease: lease,
    })
}

#[cfg(test)]
mod budget_tests {
    #[test]
    fn input_quota_lives_with_face_and_releases_on_error_or_unwind() {
        use crate::{budget::OperationBudget, error::PdfError};
        let data = crate::font::builtin_font::load_builtin_font_data("Helvetica").unwrap();
        let pool = OperationBudget::new(data.len());
        let face = super::load_face_for_operation(data, Some(&pool)).unwrap();
        assert!(face.num_glyphs() > 0);
        assert_eq!(pool.live_bytes(), data.len());
        let neighbor = OperationBudget::new(data.len());
        let held = neighbor.reserve(1).unwrap();
        assert!(matches!(
            super::load_face_for_operation(data, Some(&neighbor)),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(neighbor.live_bytes(), 1);
        drop(held);
        drop(super::load_face_for_operation(data, Some(&neighbor)).unwrap());
        assert_eq!(neighbor.live_bytes(), 0);
        drop(face);
        assert_eq!(pool.live_bytes(), 0);
        assert!(super::load_face_for_operation(b"not a font", Some(&pool)).is_err());
        assert_eq!(pool.live_bytes(), 0);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _face = super::load_face_for_operation(data, Some(&pool)).unwrap();
            panic!("face quota unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[derive(Debug)]
pub enum CharmapType {
    MsSymbol,
    MsUnicode,
    MacRoman,
    Other,
}

fn use_tt_charmap(face: &Face, platform_id: u16, encoding_id: u16) -> PdfResult<bool> {
    let num_charmaps = face.num_charmaps();
    for i in 0..num_charmaps {
        let charmap = face.get_charmap(i as isize);
        if charmap.platform_id() == platform_id && charmap.encoding_id() == encoding_id {
            face.set_charmap(&charmap)
                .map_err(|e| PdfError::FontError(format!("Freetype use chamap error:{:?}", e)))?;
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn use_charmap(is_symbolic: bool, face: &Face) -> PdfResult<CharmapType> {
    if is_symbolic && use_tt_charmap(face, 3, 0)? {
        return Ok(CharmapType::MsSymbol);
    }
    if is_symbolic && use_tt_charmap(face, 1, 0)? {
        return Ok(CharmapType::MacRoman);
    }
    if use_tt_charmap(face, 3, 1)? {
        return Ok(CharmapType::MsUnicode);
    }
    if use_tt_charmap(face, 1, 0)? {
        return Ok(CharmapType::MacRoman);
    }
    let cm = face.get_charmap(0_isize);
    face.set_charmap(&cm)
        .map_err(|e| PdfError::FontError(format!("Freetype set charmap error:{:?}", e)))?;
    Ok(CharmapType::Other)
}

// The freetype-rs bindings omit the CID service; use the linked FreeType ABI.
unsafe extern "C" {
    fn FT_Get_CID_From_Glyph_Index(
        face: freetype::ffi::FT_Face,
        glyph_index: u32,
        cid: *mut u32,
    ) -> i32;
}
pub(crate) fn cff_cid(face: &Face, glyph: u32) -> Option<u32> {
    let mut cid = 0;
    // SAFETY: face owns a live FT_Face and cid is a writable FT_UInt.
    let status = unsafe {
        FT_Get_CID_From_Glyph_Index(
            face.raw() as *const _ as freetype::ffi::FT_Face,
            glyph,
            &mut cid,
        )
    };
    (status == 0).then_some(cid)
}

#[cfg(test)]
mod symbolic_cmap_tests {
    use super::*;
    #[test]
    fn symbolic_raw_byte_prefers_mac_mapping_over_unicode_mapping() {
        // Synthetic three-glyph font: Mac byte 32 and Unicode U+00D7 both
        // select glyph 2; Unicode U+0020 deliberately has no glyph.
        let face = load_freetype_face(
            include_bytes!("../../tests/fixtures/synthetic-symbol.ttf").to_vec(),
        )
        .unwrap();
        assert!(matches!(
            use_charmap(false, &face).unwrap(),
            CharmapType::MsUnicode
        ));
        assert_eq!(face.get_char_index(32), None);
        assert_eq!(face.get_char_index(0xD7), Some(2));
        assert!(matches!(
            use_charmap(true, &face).unwrap(),
            CharmapType::MacRoman
        ));
        assert_eq!(face.get_char_index(32), Some(2));
    }
}

/// Invert a real Unicode charmap only for glyphs with one non-private scalar.
/// Symbol charmaps and generated F0xx aliases are not character semantics.
pub(crate) fn glyph_unicode_map(
    face: &Face,
    limits: crate::parser::options::Limits,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(Vec<Option<String>>, Option<crate::budget::MemoryLease>)> {
    let mut selected = None;
    for i in 0..face.num_charmaps() {
        let cm = face.get_charmap(i as isize);
        let rank = match (cm.platform_id(), cm.encoding_id()) {
            (3, 10) => 3,
            (0, _) => 2,
            (3, 1) => 1,
            _ => 0,
        };
        if rank > selected.map_or(0, |(_, rank)| rank) {
            selected = Some((i, rank));
        }
    }
    let Some((index, _)) = selected else {
        return Ok((Vec::new(), None));
    };
    face.set_charmap(&face.get_charmap(index as isize))
        .map_err(|e| PdfError::FontError(format!("Unicode charmap selection: {e:?}")))?;
    let count = usize::try_from(face.num_glyphs())
        .map_err(|_| PdfError::FontError("invalid font glyph count".into()))?;
    if count > limits.max_objects {
        return Err(PdfError::ResourceLimit("Unicode glyph entries".into()));
    }
    let peak = count
        .checked_mul(std::mem::size_of::<Option<String>>() + 8)
        .filter(|n| *n <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("Unicode glyph bytes".into()))?;
    let mut lease = budget.map(|b| b.reserve(peak)).transpose()?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("Unicode glyph allocation".into()))?;
    values.resize(count, u32::MAX);
    for (work, (code, gid)) in face.chars().enumerate() {
        if work >= limits.max_objects {
            return Err(PdfError::ResourceLimit("Unicode cmap entries".into()));
        }
        let Ok(code) = u32::try_from(code) else {
            continue;
        };
        if char::from_u32(code).is_none()
            || matches!(code,0xE000..=0xF8FF|0xF0000..=0xFFFFD|0x100000..=0x10FFFD)
        {
            continue;
        }
        let Some(value) = values.get_mut(gid.get() as usize) else {
            continue;
        };
        if *value == u32::MAX {
            *value = code;
        } else if *value != code {
            *value = u32::MAX - 1;
        }
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("Unicode glyph output allocation".into()))?;
    let mut strings = 0usize;
    for value in &values {
        let value = char::from_u32(*value).map(|c| c.to_string());
        strings += value.as_ref().map_or(0, String::capacity);
        result.push(value);
    }
    drop(values);
    if let Some(lease) = &mut lease {
        lease.resize(result.capacity() * std::mem::size_of::<Option<String>>() + strings)?;
    }
    Ok((result, lease))
}

#[cfg(test)]
mod unicode_fallback_tests {
    use super::*;
    fn synthetic_cmap_variant(ambiguous: bool) -> Vec<u8> {
        let mut bytes = include_bytes!("../../tests/fixtures/synthetic-symbol.ttf").to_vec();
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let entry = (0..count)
            .map(|i| 12 + i * 16)
            .find(|i| &bytes[*i..*i + 4] == b"cmap")
            .unwrap();
        let start = u32::from_be_bytes(bytes[entry + 8..entry + 12].try_into().unwrap()) as usize;
        let records = u16::from_be_bytes(bytes[start + 2..start + 4].try_into().unwrap()) as usize;
        let mac = (0..records)
            .map(|i| start + 4 + i * 8)
            .find(|i| bytes[*i..*i + 4] == [0, 1, 0, 0])
            .unwrap();
        let offset = u32::from_be_bytes(bytes[mac + 4..mac + 8].try_into().unwrap()) as usize;
        for i in 0..records {
            let record = start + 4 + i * 8;
            if record == mac {
                continue;
            }
            if ambiguous {
                bytes[record..record + 4].copy_from_slice(&[0, 0, 0, 3]);
                bytes[record + 4..record + 8].copy_from_slice(&(offset as u32).to_be_bytes());
            } else {
                bytes[record..record + 4].copy_from_slice(&[0, 3, 0, 0]);
            }
        }
        if ambiguous {
            bytes[start + offset + 6 + 33] = 2;
        }
        bytes
    }
    #[test]
    fn symbol_charmaps_and_ambiguous_unicode_do_not_invent_text() {
        for ambiguous in [false, true] {
            let face = load_freetype_face(synthetic_cmap_variant(ambiguous)).unwrap();
            let (mapping, _) =
                glyph_unicode_map(&face, crate::parser::options::Limits::default(), None).unwrap();
            assert!(mapping.iter().all(Option::is_none));
        }
    }
    #[test]
    fn unicode_inverse_uses_semantic_charmap_and_enforces_limits() {
        let face = load_freetype_face(
            include_bytes!("../../tests/fixtures/synthetic-symbol.ttf").to_vec(),
        )
        .unwrap();
        let (mapping, _) =
            glyph_unicode_map(&face, crate::parser::options::Limits::default(), None).unwrap();
        assert_eq!(mapping[2].as_deref(), Some("×"));
        assert_eq!(mapping[0], None);
        let limits = crate::parser::options::Limits {
            max_objects: 1,
            ..Default::default()
        };
        assert!(matches!(
            glyph_unicode_map(&face, limits, None),
            Err(PdfError::ResourceLimit(_))
        ));
        let limits = crate::parser::options::Limits {
            max_decoded_bytes: 1,
            ..Default::default()
        };
        assert!(matches!(
            glyph_unicode_map(&face, limits, None),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}

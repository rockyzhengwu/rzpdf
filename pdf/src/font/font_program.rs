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
    let lib = Library::init()
        .map_err(|e| PdfError::FontError(format!("Freetype library init error:{:?}", e)))?;
    match lib.new_memory_face(fontfile, 0) {
        Ok(face) => Ok(face),
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

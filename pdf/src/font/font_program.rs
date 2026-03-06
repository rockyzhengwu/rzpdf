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

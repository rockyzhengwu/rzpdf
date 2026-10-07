//! Usage: merge_documents OUTPUT.pdf INPUT1.pdf [INPUT2.pdf ...]
//! Page-only merge: source catalog metadata/outlines/annotations are not merged.
use pdf::{
    document::{PdfDocument, save_options::PdfVersion},
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
use std::io::{Cursor, Read, Seek};

fn merge_sources<R: Read + Seek>(
    sources: &[PdfDocument<R>],
) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
    let mut version = PdfVersion::V1_0;
    for source in sources {
        version = version.max(source.effective_version()?);
    }
    let result = PdfDocument::new_with_version(version)?;
    for source in sources {
        result.merge_document(source)?;
    }
    Ok(result)
}

fn main() -> PdfResult<()> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.len() < 2 {
        return Err(PdfError::DocumentError(
            "Usage: merge_documents OUTPUT.pdf INPUT1.pdf [INPUT2.pdf ...]".into(),
        ));
    }
    // Reuse the same opened contexts for version selection and graph copying.
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(paths.len() - 1)
        .map_err(|_| PdfError::ResourceLimit("Merge source allocation".into()))?;
    for input in &paths[1..] {
        let source = PdfDocument::open_with_mode(input, ParseMode::Strict)?;
        sources.push(source);
    }
    let result = merge_sources(&sources)?;
    let expected = result.try_page_count()?;
    result.save_to_file(&paths[0])?;
    let reopened = PdfDocument::open_with_mode(&paths[0], ParseMode::Strict)?;
    if reopened.try_page_count()? != expected {
        return Err(PdfError::DocumentError("Merged page count mismatch".into()));
    }
    for index in 0..expected {
        reopened
            .get_page(
                u32::try_from(index)
                    .map_err(|_| PdfError::ResourceLimit("Merged page index".into()))?,
            )?
            .display_list()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_versions_choose_highest_and_preserve_all_pages() {
        let first = PdfDocument::new_with_version(PdfVersion::V1_4).unwrap();
        first.add_page(0).unwrap();
        let second = PdfDocument::new_with_version(PdfVersion::V2_0).unwrap();
        second.add_page(0).unwrap();
        let revisions = [first.revision(), second.revision()];
        let sources = [first, second];
        let merged = merge_sources(&sources).unwrap();
        assert_eq!(merged.effective_version().unwrap(), PdfVersion::V2_0);
        assert_eq!(merged.try_page_count().unwrap(), 2);
        let reopened =
            PdfDocument::from_bytes_with_mode(merged.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        assert_eq!(reopened.version(), "2.0");
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        assert_eq!([sources[0].revision(), sources[1].revision()], revisions);
    }
}

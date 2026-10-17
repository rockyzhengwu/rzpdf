//! Typed markup annotations, stable CRUD and self-contained normal appearances.
mod edit;
mod model;
mod paint;
pub use model::*;

use super::PdfDocument;
use crate::error::PdfResult;
use std::io::{Read, Seek};

#[derive(Debug, Clone, PartialEq)]
pub struct SquareAnnotation {
    pub rect: [f32; 4],
    pub color: [f32; 3],
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Compatibility projection of the unified annotation query.
    pub fn page_square_annotations(&self, page: u32) -> PdfResult<Vec<SquareAnnotation>> {
        Ok(self
            .page_annotations(page)?
            .into_iter()
            .filter_map(|record| {
                record.annotation.and_then(|a| {
                    matches!(a.kind, AnnotationKind::Square).then_some(SquareAnnotation {
                        rect: a.rect,
                        color: a.color,
                    })
                })
            })
            .collect())
    }

    pub fn add_square_annotation(
        &self,
        page: u32,
        rect: [f32; 4],
        color: [f32; 3],
    ) -> PdfResult<()> {
        let mut annotation = Annotation::new(AnnotationKind::Square, rect);
        annotation.color = color;
        self.add_annotation(page, &annotation).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_appearance_roundtrip() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let expected = SquareAnnotation {
            rect: [10.0, 20.0, 80.0, 40.0],
            color: [1.0, 0.0, 0.0],
        };
        doc.add_square_annotation(0, expected.rect, expected.color)
            .unwrap();
        let revision = doc.commit_revision();
        assert!(
            doc.add_square_annotation(0, expected.rect, [2.0, 0.0, 0.0])
                .is_err()
        );
        assert_eq!(doc.commit_revision(), revision);
        assert_eq!(
            doc.page_square_annotations(0).unwrap(),
            vec![expected.clone()]
        );
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_square_annotations(0).unwrap(), vec![expected]);
    }
}

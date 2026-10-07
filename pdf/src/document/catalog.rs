use super::page_tree::PageTree;
use crate::{
    error::PdfResult,
    objects::{PdfObject, pdf_reference::PdfReference},
    pdf_context::PDFContext,
};
use std::io::{Read, Seek};

#[derive(Debug, Default)]
pub struct Catalog;

impl Catalog {
    pub fn try_new<R: Seek + Read>(_ctx: &PDFContext<R>) -> PdfResult<Self> {
        Ok(Self)
    }

    pub fn get_page<R: Seek + Read>(
        &self,
        pagenum: u32,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Option<PdfObject>> {
        Ok(PageTree::entries(ctx)?
            .get(pagenum as usize)
            .map(|entry| PdfObject::PdfReference(PdfReference::from_id(entry.page_id))))
    }

    pub fn total_page<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<usize> {
        Ok(PageTree::entries(ctx)?.len())
    }
}

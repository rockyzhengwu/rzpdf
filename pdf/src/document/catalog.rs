use std::{
    collections::VecDeque,
    io::{Read, Seek},
};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Default)]
pub struct Catalog;

impl Catalog {
    pub fn try_new<R: Seek + Read>(_ctx: &PDFContext<R>) -> PdfResult<Self> {
        Ok(Self)
    }

    pub fn add_page(&mut self) {
        unimplemented!()
    }

    pub fn delete_page(&mut self) {
        unimplemented!()
    }

    pub fn get_page<'a, R: Seek + Read>(
        &'a self,
        pagenum: u32,
        ctx: &'a PDFContext<R>,
    ) -> PdfResult<Option<PdfObject>> {
        let pages = page_nodes(ctx)?;
        let Some(page_node) = pages.get(pagenum as usize) else {
            return Ok(None);
        };
        let _ = ctx
            .resolve_owned(page_node)?
            .into_dict()
            .ok_or(PdfError::DocumentError(format!(
                "page node {pagenum} is not a dict"
            )))?;
        Ok(Some(page_node.clone()))
    }

    pub fn total_page<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<usize> {
        Ok(page_nodes(ctx)?.len())
    }
}

fn page_nodes<R: Seek + Read>(ctx: &PDFContext<R>) -> PdfResult<Vec<PdfObject>> {
    let root = ctx.get_root_owned()?;
    let pages = root.get("Pages").ok_or(PdfError::DocumentError(
        "document catalog has no Pages attribute".to_string(),
    ))?;
    get_pages(ctx.resolve_owned(pages)?, ctx)
}

fn get_pages<R: Seek + Read>(root: PdfObject, ctx: &PDFContext<R>) -> PdfResult<Vec<PdfObject>> {
    let mut stack = VecDeque::new();
    stack.push_back(root);
    let mut pages = Vec::new();
    while let Some(node) = stack.pop_front() {
        let resolved_node = ctx.resolve_owned(&node)?;
        let nodetype = resolved_node
            .get_attr("Type")
            .ok_or(PdfError::DocumentError("page tree node missing Type".to_string()))?
            .as_name()
            .ok_or(PdfError::DocumentError(
                "page tree node Type is not a name".to_string(),
            ))?
            .name();
        if nodetype == "Pages" {
            let kids = resolved_node
                .get_attr("Kids")
                .ok_or(PdfError::DocumentError("Pages node missing Kids".to_string()))?
                .as_array()
                .ok_or(PdfError::DocumentError(
                    "Pages node Kids is not an array".to_string(),
                ))?;
            for kid in kids {
                stack.push_back(kid.clone());
            }
        } else if nodetype == "Page" {
            pages.push(node);
        }
    }
    Ok(pages)
}

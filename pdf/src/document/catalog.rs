use std::{
    collections::VecDeque,
    io::{Read, Seek},
};

use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
    pdf_context::PDFContext,
};

#[derive(Debug)]
pub struct Catalog {
    root: PdfDict,
    pages: Vec<PdfDict>,
}

impl Catalog {
    pub fn try_new<R: Seek + Read>(ctx: &PDFContext<R>) -> PdfResult<Self> {
        let root = ctx.get_root()?;
        let pages = root.get("Pages").ok_or(PdfError::DocumentError(
            "get_page_tree Document Catalog has no pages attribute".to_string(),
        ))?;
        let page_root = ctx.resolve(pages)?.as_dict().unwrap();
        let pages = get_pages(page_root, ctx)?;
        Ok(Self {
            root: page_root.to_owned(),
            pages,
        })
    }

    pub fn add_page(&mut self) {
        unimplemented!()
    }

    pub fn delete_page(&mut self) {
        unimplemented!()
    }

    pub fn get_page(&self, pagenum: u32) -> Option<&PdfDict> {
        if pagenum >= self.pages.len() as u32 {
            println!("None");
            return None;
        }
        Some(&self.pages[pagenum as usize])
    }

    pub fn total_page(&self) -> usize {
        self.pages.len()
    }
}

fn get_pages<R: Seek + Read>(root: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Vec<PdfDict>> {
    let mut stack = VecDeque::new();
    stack.push_back(root);
    let mut pages = Vec::new();
    while !stack.is_empty() {
        let node = stack.pop_front().unwrap();
        let nodetype = node.get("Type").unwrap().as_name().unwrap().name();
        if nodetype == "Pages" {
            let kids = node.get("Kids").unwrap().as_array().unwrap();
            for kid in kids.into_iter() {
                let kid_dict = ctx.resolve(kid)?;
                stack.push_back(kid_dict.as_dict().unwrap());
            }
        } else if nodetype == "Page" {
            pages.push(node.to_owned());
        }
    }
    Ok(pages)
}

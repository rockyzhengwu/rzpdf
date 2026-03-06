use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
};

use crate::{
    document::catalog::Catalog,
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    page::PdfPage,
    parser::{
        parser::{PdfParser, parse_xref},
        syntax::SyntaxParser,
    },
    pdf_context::PDFContext,
};

pub mod catalog;
pub mod cross_ref_table;
pub mod page_tree;

pub struct PdfDocument<R: Seek + Read> {
    ctx: PDFContext<R>,
    catalog: Catalog,
}

impl PdfDocument<BufReader<File>> {
    pub fn load<P: AsRef<Path>>(path: P) -> PdfResult<PdfDocument<BufReader<File>>> {
        let file = File::open(path)
            .map_err(|e| PdfError::DocumentError(format!("load document failed:{:?}", e)))?;
        let reader = BufReader::new(file);
        let stream_reader = StreamReader::try_new(reader)?;
        let mut syntax = SyntaxParser::new(stream_reader);
        let cross_ref_table = parse_xref(&mut syntax)?;
        let parser = PdfParser::new(syntax);
        let mut ctx = PDFContext::try_new(parser, cross_ref_table)?;
        ctx.parse_indirect_objects()?;
        let catalog = Catalog::try_new(&ctx)?;
        let doc = PdfDocument { ctx: ctx, catalog };
        Ok(doc)
    }
}
impl PdfDocument<Cursor<Vec<u8>>> {
    pub fn load(buffer: Vec<u8>) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        let reader = StreamReader::try_new(Cursor::new(buffer))?;
        let mut syntax = SyntaxParser::new(reader);
        let cross_ref_table = parse_xref(&mut syntax)?;
        let parser = PdfParser::new(syntax);
        let ctx = PDFContext::try_new(parser, cross_ref_table)?;
        let catalog = Catalog::try_new(&ctx)?;
        Ok(PdfDocument { ctx, catalog })
    }
}

impl<R: Seek + Read> PdfDocument<R> {
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn add_page(&self, pagenum: u32) -> PdfResult<()> {
        // 1. create a new pagedict insert to context indirect_object and add pagenode to pagetree
        //    in catalog;
        // 2. return Page object
        unimplemented!()
    }

    pub fn get_page(&self, page: u32) -> PdfResult<PdfPage<'_, R>> {
        if let Some(pagenode) = self.catalog.get_page(page) {
            let page = PdfPage::new(pagenode, &self.ctx);
            return Ok(page);
        }
        return Err(PdfError::PageNotExist);
    }
}

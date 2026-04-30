use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
};

use crate::{
    document::catalog::Catalog,
    error::{PdfError, PdfResult},
    geom::rectangle::Rectangle,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict,
        pdf_name::PdfName, pdf_number::PdfNumber, pdf_reference::PdfReference,
        pdf_stream::PdfStream,
    },
    page::{PdfPage, content_builder::ContentBuilder},
    parser::{
        parser::{PdfParser, parse_xref},
        syntax::SyntaxParser,
    },
    pdf_context::PDFContext,
};

pub mod catalog;
pub mod cross_ref_table;
pub mod page_tree;
pub mod store;

pub struct PdfDocument<R: Seek + Read> {
    ctx: PDFContext<R>,
    catalog: Catalog,
}

impl PdfDocument<BufReader<File>> {
    pub fn open<P: AsRef<Path>>(path: P) -> PdfResult<PdfDocument<BufReader<File>>> {
        Self::open_with_mode(path, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn open_with_mode<P: AsRef<Path>>(
        path: P,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<BufReader<File>>> {
        let file = File::open(path)
            .map_err(|e| PdfError::DocumentError(format!("load document failed:{:?}", e)))?;
        let reader = BufReader::new(file);
        Self::from_reader_with_mode(reader, mode)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> PdfResult<PdfDocument<BufReader<File>>> {
        Self::open(path)
    }
}
impl PdfDocument<Cursor<Vec<u8>>> {
    pub fn from_bytes(buffer: Vec<u8>) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_bytes_with_mode(buffer, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn from_bytes_with_mode(
        buffer: Vec<u8>,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_reader_with_mode(Cursor::new(buffer), mode)
    }

    pub fn load(buffer: Vec<u8>) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_bytes(buffer)
    }
}

impl<R: Seek + Read> PdfDocument<R> {
    pub(crate) fn ctx(&self) -> &PDFContext<R> {
        &self.ctx
    }

    pub fn from_reader(reader: R) -> PdfResult<PdfDocument<R>> {
        Self::from_reader_with_mode(reader, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn from_reader_with_mode(
        reader: R,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<R>> {
        let reader = StreamReader::try_new(reader)?;
        let mut syntax = SyntaxParser::with_mode(reader, mode);
        let _ = syntax.read_header()?;
        let cross_ref_table = parse_xref(&mut syntax)?;
        let parser = PdfParser::new(syntax.with_xref_table(cross_ref_table.clone()));
        let ctx = PDFContext::try_new(parser, cross_ref_table)?;
        let catalog = Catalog::try_new(&ctx)?;
        Ok(PdfDocument { ctx, catalog })
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn page_count(&self) -> usize {
        self.catalog.total_page(&self.ctx).unwrap_or(0)
    }

    pub fn resolve_object(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        self.ctx.resolve_object_owned(object_id)
    }

    pub fn insert_object(&self, object: PdfObject) -> ObjectId {
        self.ctx.insert_object(object)
    }

    pub fn update_object(&self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        self.ctx.update_object(object_id, object)
    }

    pub fn delete_object(&self, object_id: ObjectId) -> PdfResult<()> {
        self.ctx.delete_object(object_id)
    }

    pub fn add_page(&self, pagenum: u32) -> PdfResult<()> {
        let page_count = self.page_count() as u32;
        if pagenum > page_count {
            return Err(PdfError::PageNotExist);
        }

        let insertion = self.page_insertion_point(pagenum)?;
        let new_page_id = self.create_empty_page(insertion.parent_id)?;
        self.update_pages_kids(
            insertion.parent_id,
            insertion.kid_index,
            PdfObject::PdfReference(PdfReference::from_id(new_page_id)),
            false,
        )?;
        self.adjust_page_counts(&insertion.ancestor_ids, 1)?;
        Ok(())
    }

    pub fn delete_page(&self, pagenum: u32) -> PdfResult<()> {
        let page_count = self.page_count() as u32;
        if pagenum >= page_count {
            return Err(PdfError::PageNotExist);
        }

        let location = self.page_location(pagenum)?;
        self.update_pages_kids(location.parent_id, location.kid_index, PdfObject::PdfNull, true)?;
        self.adjust_page_counts(&location.ancestor_ids, -1)?;
        self.delete_object(location.page_id)?;
        self.delete_page_content_streams(&location.page_dict)?;
        Ok(())
    }

    pub fn replace_page_contents(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        let location = self.page_location(pagenum)?;
        let mut page_dict = location.page_dict;
        self.delete_page_content_streams(&page_dict)?;
        let stream_id = self.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            content,
        )));
        page_dict.insert(
            "Contents".to_string(),
            PdfObject::PdfReference(PdfReference::from_id(stream_id)),
        );
        self.update_object(location.page_id, PdfObject::PdfDict(page_dict))?;
        Ok(())
    }

    pub fn replace_page_content_builder(
        &self,
        pagenum: u32,
        builder: &ContentBuilder,
    ) -> PdfResult<()> {
        self.replace_page_contents(pagenum, builder.build())
    }

    pub fn append_page_contents(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        let location = self.page_location(pagenum)?;
        let mut page_dict = location.page_dict;
        let stream_id = self.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            content,
        )));
        let new_ref = PdfObject::PdfReference(PdfReference::from_id(stream_id));
        let contents = match page_dict.get("Contents").cloned() {
            Some(PdfObject::PdfArray(mut items)) => {
                items.add_obj(new_ref);
                PdfObject::PdfArray(items)
            }
            Some(existing) => {
                let mut items = PdfArray::default();
                items.add_obj(existing);
                items.add_obj(new_ref);
                PdfObject::PdfArray(items)
            }
            None => new_ref,
        };
        page_dict.insert("Contents".to_string(), contents);
        self.update_object(location.page_id, PdfObject::PdfDict(page_dict))?;
        Ok(())
    }

    pub fn append_page_content_builder(
        &self,
        pagenum: u32,
        builder: &ContentBuilder,
    ) -> PdfResult<()> {
        self.append_page_contents(pagenum, builder.build())
    }

    pub fn get_page(&self, page: u32) -> PdfResult<PdfPage<'_, R>> {
        if let Some(pagenode) = self.catalog.get_page(page, &self.ctx)? {
            let page = PdfPage::new(pagenode, &self.ctx);
            return Ok(page);
        }
        Err(PdfError::PageNotExist)
    }
}

#[cfg(test)]
mod tests {
    use super::PdfDocument;
    use crate::parser::syntax::ParseMode;

    #[test]
    fn test_from_bytes_with_strict_mode_rejects_missing_binary_comment() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< /Size 1 >>\nstartxref\n20\n%%EOF\n";
        assert!(PdfDocument::from_bytes_with_mode(pdf.to_vec(), ParseMode::Strict).is_err());
    }
}

impl<R: Seek + Read> PdfDocument<R> {
    fn root_pages(&self) -> PdfResult<(ObjectId, PdfDict)> {
        let root_ref = self
            .ctx
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_reference)
            .ok_or(PdfError::DocumentError(
                "document Root must be an indirect reference".to_string(),
            ))?;
        let root_dict = self
            .resolve_object(root_ref.id())?
            .into_dict()
            .ok_or(PdfError::DocumentError("document Root is not a dict".to_string()))?;
        let pages_ref = root_dict
            .get("Pages")
            .cloned()
            .and_then(PdfObject::into_reference)
            .ok_or(PdfError::DocumentError(
                "document Pages root must be an indirect reference".to_string(),
            ))?;
        let pages_id = pages_ref.id();
        let pages_dict = self
            .resolve_object(pages_id)?
            .into_dict()
            .ok_or(PdfError::DocumentError("Pages root is not a dict".to_string()))?;
        Ok((pages_id, pages_dict))
    }

    fn create_empty_page(&self, parent_id: ObjectId) -> PdfResult<ObjectId> {
        let contents_id = self.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            Vec::new(),
        )));
        let page = new_page_dict(parent_id, contents_id);
        Ok(self.insert_object(PdfObject::PdfDict(page)))
    }

    fn page_insertion_point(&self, pagenum: u32) -> PdfResult<PageInsertionPoint> {
        let entries = self.collect_page_entries()?;
        if entries.is_empty() {
            let (root_id, _) = self.root_pages()?;
            return Ok(PageInsertionPoint {
                parent_id: root_id,
                kid_index: 0,
                ancestor_ids: vec![root_id],
            });
        }

        if (pagenum as usize) < entries.len() {
            let target = &entries[pagenum as usize];
            return Ok(PageInsertionPoint {
                parent_id: target.parent_id,
                kid_index: target.kid_index,
                ancestor_ids: target.ancestor_ids.clone(),
            });
        }

        let target = entries
            .last()
            .ok_or(PdfError::DocumentError("page tree is empty".to_string()))?;
        Ok(PageInsertionPoint {
            parent_id: target.parent_id,
            kid_index: target.kid_index + 1,
            ancestor_ids: target.ancestor_ids.clone(),
        })
    }

    fn page_location(&self, pagenum: u32) -> PdfResult<PageTreeEntry> {
        let entries = self.collect_page_entries()?;
        entries
            .into_iter()
            .nth(pagenum as usize)
            .ok_or(PdfError::PageNotExist)
    }

    fn collect_page_entries(&self) -> PdfResult<Vec<PageTreeEntry>> {
        let (root_id, root_dict) = self.root_pages()?;
        let mut pages = Vec::new();
        let mut ancestors = vec![root_id];
        self.collect_page_entries_from_node(root_id, root_dict, &mut ancestors, &mut pages)?;
        Ok(pages)
    }

    fn collect_page_entries_from_node(
        &self,
        node_id: ObjectId,
        node_dict: PdfDict,
        ancestors: &mut Vec<ObjectId>,
        pages: &mut Vec<PageTreeEntry>,
    ) -> PdfResult<()> {
        let kids = node_dict
            .get("Kids")
            .and_then(PdfObject::as_array)
            .ok_or(PdfError::DocumentError(
                "Pages node Kids must be an array".to_string(),
            ))?;

        for (kid_index, kid) in kids.into_iter().enumerate() {
            let kid_ref = kid.as_reference().ok_or(PdfError::DocumentError(
                "page tree mutation requires indirect Kids references".to_string(),
            ))?;
            let kid_id = kid_ref.id();
            let kid_dict = self
                .resolve_object(kid_id)?
                .into_dict()
                .ok_or(PdfError::DocumentError(
                    "page tree kid is not a dict".to_string(),
                ))?;
            let node_type = kid_dict
                .get("Type")
                .and_then(PdfObject::as_name)
                .map(|name| name.name().to_string())
                .ok_or(PdfError::DocumentError(
                    "page tree node missing Type".to_string(),
                ))?;

            if node_type == "Pages" {
                ancestors.push(kid_id);
                self.collect_page_entries_from_node(kid_id, kid_dict, ancestors, pages)?;
                ancestors.pop();
            } else if node_type == "Page" {
                pages.push(PageTreeEntry {
                    page_id: kid_id,
                    page_dict: kid_dict,
                    parent_id: node_id,
                    kid_index,
                    ancestor_ids: ancestors.clone(),
                });
            } else {
                return Err(PdfError::DocumentError(format!(
                    "unsupported page tree node type: {node_type}"
                )));
            }
        }
        Ok(())
    }

    fn update_pages_kids(
        &self,
        pages_id: ObjectId,
        kid_index: usize,
        kid: PdfObject,
        remove: bool,
    ) -> PdfResult<()> {
        let mut pages_dict = self
            .resolve_object(pages_id)?
            .into_dict()
            .ok_or(PdfError::DocumentError("Pages node is not a dict".to_string()))?;
        let mut kids = pages_dict
            .get("Kids")
            .cloned()
            .and_then(PdfObject::into_array)
            .ok_or(PdfError::DocumentError(
                "Pages node Kids must be an array".to_string(),
            ))?;
        if remove {
            let _ = kids.remove(kid_index).ok_or(PdfError::PageNotExist)?;
        } else {
            kids.insert(kid_index, kid);
        }
        pages_dict.insert("Kids".to_string(), PdfObject::PdfArray(kids));
        self.update_object(pages_id, PdfObject::PdfDict(pages_dict))
    }

    fn adjust_page_counts(&self, ancestor_ids: &[ObjectId], delta: i32) -> PdfResult<()> {
        for object_id in ancestor_ids {
            let mut pages_dict = self
                .resolve_object(*object_id)?
                .into_dict()
                .ok_or(PdfError::DocumentError("Pages node is not a dict".to_string()))?;
            let current = pages_dict
                .get("Count")
                .and_then(PdfObject::as_i32)
                .ok_or(PdfError::DocumentError(
                    "Pages node Count must be a number".to_string(),
                ))?;
            let next = current + delta;
            if next < 0 {
                return Err(PdfError::DocumentError(
                    "page tree Count cannot be negative".to_string(),
                ));
            }
            pages_dict.insert(
                "Count".to_string(),
                PdfObject::PdfNumber(PdfNumber::new(next as f32)),
            );
            self.update_object(*object_id, PdfObject::PdfDict(pages_dict))?;
        }
        Ok(())
    }

    fn delete_page_content_streams(&self, page_dict: &PdfDict) -> PdfResult<()> {
        for object_id in page_contents_object_ids(page_dict) {
            self.delete_object(object_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PageTreeEntry {
    page_id: ObjectId,
    page_dict: PdfDict,
    parent_id: ObjectId,
    kid_index: usize,
    ancestor_ids: Vec<ObjectId>,
}

#[derive(Debug, Clone)]
struct PageInsertionPoint {
    parent_id: ObjectId,
    kid_index: usize,
    ancestor_ids: Vec<ObjectId>,
}

fn new_page_dict(parent_id: ObjectId, contents_id: ObjectId) -> PdfDict {
    let mut page = PdfDict::default();
    page.insert(
        "Type".to_string(),
        PdfObject::PdfName(PdfName::new("Page".to_string())),
    );
    page.insert(
        "Parent".to_string(),
        PdfObject::PdfReference(PdfReference::from_id(parent_id)),
    );
    page.insert(
        "MediaBox".to_string(),
        PdfObject::PdfArray(rect_to_array(Rectangle::new_a4())),
    );
    page.insert(
        "Resources".to_string(),
        PdfObject::PdfDict(PdfDict::default()),
    );
    page.insert(
        "Contents".to_string(),
        PdfObject::PdfReference(PdfReference::from_id(contents_id)),
    );
    page
}

fn page_contents_object_ids(page_dict: &PdfDict) -> Vec<ObjectId> {
    let Some(contents) = page_dict.get("Contents") else {
        return Vec::new();
    };
    match contents {
        PdfObject::PdfReference(reference) => vec![reference.id()],
        PdfObject::PdfArray(items) => items
            .into_iter()
            .filter_map(PdfObject::as_reference)
            .map(PdfReference::id)
            .collect(),
        _ => Vec::new(),
    }
}

fn rect_to_array(rect: Rectangle) -> PdfArray {
    let mut array = PdfArray::default();
    array.add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.llx())));
    array.add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.lly())));
    array.add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.urx())));
    array.add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.ury())));
    array
}

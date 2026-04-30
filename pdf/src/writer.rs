use std::fmt::Write as _;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::document::PdfDocument;
use crate::error::{PdfError, PdfResult};
use crate::objects::{
    PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
    pdf_stream::PdfStream, pdf_string::PdfString,
};

pub fn save_document<R: std::io::Read + std::io::Seek>(document: &PdfDocument<R>) -> PdfResult<Vec<u8>> {
    document.save_to_bytes()
}

pub(crate) fn write_document<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
) -> PdfResult<Vec<u8>> {
    document.ctx().materialize_all_objects()?;

    let mut output = Vec::new();
    output.extend_from_slice(b"%PDF-1.7\n");

    let object_ids = document.ctx().serializable_object_ids();
    let max_obj_num = object_ids.iter().map(|id| id.obj_num()).max().unwrap_or(0);
    let mut xref_entries = vec![(0_u64, 0_u32, b'f'); (max_obj_num as usize) + 1];
    if !xref_entries.is_empty() {
        xref_entries[0] = (0, 65535, b'f');
    }

    for object_id in object_ids {
        if let Some(obj) = document.ctx().object_for_write(object_id)? {
            xref_entries[object_id.obj_num() as usize] =
                (output.len() as u64, object_id.gen_num(), b'n');
            write_indirect_object(&mut output, object_id, &obj)?;
        }
    }

    let xref_offset = output.len() as u64;
    write_xref_table(&mut output, &xref_entries);
    write_trailer(&mut output, document.ctx().trailer(), max_obj_num + 1)?;
    output.extend_from_slice(b"startxref\n");
    output.extend_from_slice(format!("{xref_offset}\n").as_bytes());
    output.extend_from_slice(b"%%EOF\n");
    Ok(output)
}

fn write_indirect_object(output: &mut Vec<u8>, object_id: ObjectId, obj: &PdfObject) -> PdfResult<()> {
    output.extend_from_slice(
        format!("{} {} obj\n", object_id.obj_num(), object_id.gen_num()).as_bytes(),
    );
    write_object(output, obj)?;
    output.extend_from_slice(b"\nendobj\n");
    Ok(())
}

fn write_xref_table(output: &mut Vec<u8>, entries: &[(u64, u32, u8)]) {
    output.extend_from_slice(b"xref\n");
    output.extend_from_slice(format!("0 {}\n", entries.len()).as_bytes());
    for (offset, gen_num, state) in entries {
        output.extend_from_slice(format!("{offset:010} {gen_num:05} {} \n", *state as char).as_bytes());
    }
}

fn write_trailer(output: &mut Vec<u8>, original: &PdfDict, size: u32) -> PdfResult<()> {
    output.extend_from_slice(b"trailer\n");
    let mut trailer = PdfDict::default();
    for (key, value) in original {
        if matches!(
            key.as_str(),
            "Prev" | "XRefStm" | "Type" | "W" | "Index" | "Filter" | "DecodeParms" | "Length"
        ) {
            continue;
        }
        trailer.insert(key.clone(), value.clone());
    }
    trailer.insert(
        "Size".to_string(),
        PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(size as f32)),
    );
    write_dict(output, &trailer)?;
    output.extend_from_slice(b"\n");
    Ok(())
}

fn write_object(output: &mut Vec<u8>, obj: &PdfObject) -> PdfResult<()> {
    match obj {
        PdfObject::PdfNull => output.extend_from_slice(b"null"),
        PdfObject::PdfNumber(number) => output.extend_from_slice(format_number(number).as_bytes()),
        PdfObject::PdfBool(value) => {
            output.extend_from_slice(if value.value() { b"true" } else { b"false" })
        }
        PdfObject::PdfName(name) => write_name(output, name),
        PdfObject::PdfString(string) => write_string(output, string),
        PdfObject::PdfArray(array) => write_array(output, array)?,
        PdfObject::PdfDict(dict) => write_dict(output, dict)?,
        PdfObject::PdfStream(stream) => write_stream(output, stream)?,
        PdfObject::PdfReference(reference) => {
            output.extend_from_slice(format!("{} {} R", reference.objnum(), reference.gennum()).as_bytes())
        }
    }
    Ok(())
}

fn write_array(output: &mut Vec<u8>, array: &PdfArray) -> PdfResult<()> {
    output.extend_from_slice(b"[");
    let mut first = true;
    for item in array {
        if !first {
            output.extend_from_slice(b" ");
        }
        first = false;
        write_object(output, item)?;
    }
    output.extend_from_slice(b"]");
    Ok(())
}

fn write_dict(output: &mut Vec<u8>, dict: &PdfDict) -> PdfResult<()> {
    output.extend_from_slice(b"<<");
    for (key, value) in dict {
        output.extend_from_slice(b"\n");
        write_name(output, &PdfName::new(key.clone()));
        output.extend_from_slice(b" ");
        write_object(output, value)?;
    }
    output.extend_from_slice(b"\n>>");
    Ok(())
}

fn write_stream(output: &mut Vec<u8>, stream: &PdfStream) -> PdfResult<()> {
    let mut dict = stream.dict().clone();
    dict.insert(
        "Length".to_string(),
        PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(
            stream.raw_data().len() as f32,
        )),
    );
    write_dict(output, &dict)?;
    output.extend_from_slice(b"\nstream\n");
    output.extend_from_slice(stream.raw_data());
    output.extend_from_slice(b"\nendstream");
    Ok(())
}

fn write_name(output: &mut Vec<u8>, name: &PdfName) {
    output.extend_from_slice(b"/");
    output.extend_from_slice(name.name().as_bytes());
}

fn write_string(output: &mut Vec<u8>, string: &PdfString) {
    output.extend_from_slice(b"(");
    for byte in string.bytes() {
        match byte {
            b'(' | b')' | b'\\' => {
                output.push(b'\\');
                output.push(*byte);
            }
            b'\n' => output.extend_from_slice(b"\\n"),
            b'\r' => output.extend_from_slice(b"\\r"),
            b'\t' => output.extend_from_slice(b"\\t"),
            b'\x08' => output.extend_from_slice(b"\\b"),
            b'\x0c' => output.extend_from_slice(b"\\f"),
            _ => output.push(*byte),
        }
    }
    output.extend_from_slice(b")");
}

fn format_number(number: &crate::objects::pdf_number::PdfNumber) -> String {
    let value = number.value();
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        let mut s = String::new();
        let _ = write!(&mut s, "{value}");
        s
    }
}

impl<R: std::io::Read + std::io::Seek> PdfDocument<R> {
    pub fn save_to_bytes(&self) -> PdfResult<Vec<u8>> {
        write_document(self)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> PdfResult<()> {
        let bytes = self.save_to_bytes()?;
        let mut file = File::create(path)
            .map_err(|e| PdfError::IOError(format!("create output file failed: {e}")))?;
        file.write_all(&bytes)
            .map_err(|e| PdfError::IOError(format!("write output file failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        document::PdfDocument,
        objects::{
            PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
            pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
        },
        page::content_builder::ContentBuilder,
    };

    fn nest_root_page_tree<R: std::io::Read + std::io::Seek>(doc: &PdfDocument<R>) {
        let root_ref = doc
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_reference)
            .unwrap();
        let root_dict = doc.resolve_object(root_ref.id()).unwrap().into_dict().unwrap();
        let root_pages_ref = root_dict
            .get("Pages")
            .cloned()
            .and_then(PdfObject::into_reference)
            .unwrap();
        let root_pages_id = root_pages_ref.id();
        let mut root_pages = doc.resolve_object(root_pages_id).unwrap().into_dict().unwrap();
        let first_page_ref = root_pages
            .get("Kids")
            .and_then(PdfObject::as_array)
            .and_then(|kids| kids.get(0))
            .and_then(PdfObject::as_reference)
            .cloned()
            .unwrap();
        let first_page_id = first_page_ref.id();

        let mut nested_pages = PdfDict::default();
        nested_pages.insert(
            "Type".to_string(),
            PdfObject::PdfName(PdfName::new("Pages".to_string())),
        );
        nested_pages.insert(
            "Parent".to_string(),
            PdfObject::PdfReference(PdfReference::from_id(root_pages_id)),
        );
        nested_pages.insert(
            "Count".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(1.0)),
        );
        let mut nested_kids = PdfArray::default();
        nested_kids.add_obj(PdfObject::PdfReference(first_page_ref));
        nested_pages.insert("Kids".to_string(), PdfObject::PdfArray(nested_kids));
        let nested_pages_id = doc.insert_object(PdfObject::PdfDict(nested_pages));

        let mut first_page = doc.resolve_object(first_page_id).unwrap().into_dict().unwrap();
        first_page.insert(
            "Parent".to_string(),
            PdfObject::PdfReference(PdfReference::from_id(nested_pages_id)),
        );
        doc.update_object(first_page_id, PdfObject::PdfDict(first_page))
            .unwrap();

        let mut root_kids = PdfArray::default();
        root_kids.add_obj(PdfObject::PdfReference(PdfReference::from_id(
            nested_pages_id,
        )));
        root_pages.insert("Kids".to_string(), PdfObject::PdfArray(root_kids));
        doc.update_object(root_pages_id, PdfObject::PdfDict(root_pages))
            .unwrap();
    }

    #[test]
    fn test_save_to_bytes_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(doc.page_count(), saved.page_count());
    }

    #[test]
    fn test_insert_object_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let object_id = doc.insert_object(PdfObject::PdfString(PdfString::new(
            b"hello".to_vec(),
            false,
        )));
        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        let object = saved.resolve_object(object_id).unwrap();
        assert_eq!(
            object,
            PdfObject::PdfString(PdfString::new(b"hello".to_vec(), false))
        );
    }

    #[test]
    fn test_update_object_visible_before_save() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let object_id = doc.insert_object(PdfObject::PdfString(PdfString::new(
            b"before".to_vec(),
            false,
        )));
        doc.update_object(
            object_id,
            PdfObject::PdfString(PdfString::new(b"after".to_vec(), false)),
        )
        .unwrap();
        let object = doc.resolve_object(object_id).unwrap();
        assert_eq!(
            object,
            PdfObject::PdfString(PdfString::new(b"after".to_vec(), false))
        );
    }

    #[test]
    fn test_delete_object_visible_before_save() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let object_id = doc.insert_object(PdfObject::PdfString(PdfString::new(
            b"gone".to_vec(),
            false,
        )));
        doc.delete_object(object_id).unwrap();
        assert!(doc.resolve_object(object_id).is_err());
    }

    #[test]
    fn test_add_page_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let before = doc.page_count();
        doc.add_page(before as u32).unwrap();
        assert_eq!(doc.page_count(), before + 1);

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(saved.page_count(), before + 1);
        assert!(saved.get_page(before as u32).is_ok());
    }

    #[test]
    fn test_delete_page_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let before = doc.page_count();
        doc.delete_page((before - 1) as u32).unwrap();
        assert_eq!(doc.page_count(), before - 1);

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(saved.page_count(), before - 1);
        assert!(saved.get_page((before - 1) as u32).is_err());
    }

    #[test]
    fn test_add_page_into_nested_tree_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        nest_root_page_tree(&doc);

        doc.add_page(0).unwrap();
        assert_eq!(doc.page_count(), 2);
        let page = doc.get_page(0).unwrap();
        assert!(page.resource().is_ok());

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(saved.page_count(), 2);
        assert!(saved.get_page(0).is_ok());
        assert!(saved.get_page(1).is_ok());
    }

    #[test]
    fn test_delete_page_from_nested_tree_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        nest_root_page_tree(&doc);
        doc.add_page(1).unwrap();
        assert_eq!(doc.page_count(), 2);

        doc.delete_page(0).unwrap();
        assert_eq!(doc.page_count(), 1);

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(saved.page_count(), 1);
        assert!(saved.get_page(0).is_ok());
        assert!(saved.get_page(1).is_err());
    }

    #[test]
    fn test_append_and_replace_page_contents_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let new_page_index = doc.page_count() as u32;
        doc.add_page(new_page_index).unwrap();
        doc.append_page_contents(new_page_index, b"q\nQ\n".to_vec())
            .unwrap();
        doc.replace_page_contents(new_page_index, b"q\nq\nQ\nQ\n".to_vec())
            .unwrap();

        let mut page = doc.get_page(new_page_index).unwrap();
        assert!(page.resource().is_ok());
        assert!(page.display_list().is_ok());

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        let mut saved_page = saved.get_page(new_page_index).unwrap();
        assert!(saved_page.resource().is_ok());
        assert!(saved_page.display_list().is_ok());
    }

    #[test]
    fn test_content_builder_roundtrip() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let page_index = doc.page_count() as u32;
        doc.add_page(page_index).unwrap();

        let mut builder = ContentBuilder::new();
        builder
            .save_state()
            .set_fill_rgb(1.0, 0.0, 0.0)
            .rectangle(10.0, 10.0, 100.0, 50.0)
            .fill()
            .restore_state();
        doc.replace_page_content_builder(page_index, &builder).unwrap();

        let mut page = doc.get_page(page_index).unwrap();
        assert!(page.display_list().is_ok());

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        let mut saved_page = saved.get_page(page_index).unwrap();
        assert!(saved_page.display_list().is_ok());
    }

    #[test]
    fn test_replace_page_contents_deletes_old_streams() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let page_index = doc.page_count() as u32;
        doc.add_page(page_index).unwrap();
        doc.append_page_contents(page_index, b"q\nQ\n".to_vec())
            .unwrap();

        let page_ref = doc
            .catalog()
            .get_page(page_index, doc.ctx())
            .unwrap()
            .unwrap()
            .into_reference()
            .unwrap();
        let page_id = page_ref.id();
        let page_dict = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
        let old_stream_ids: Vec<_> = page_dict
            .get("Contents")
            .and_then(PdfObject::as_array)
            .unwrap()
            .into_iter()
            .filter_map(PdfObject::as_reference)
            .map(|reference| reference.id())
            .collect();
        assert_eq!(old_stream_ids.len(), 2);

        doc.replace_page_contents(page_index, b"q\nq\nQ\nQ\n".to_vec())
            .unwrap();

        for object_id in old_stream_ids {
            assert!(doc.resolve_object(object_id).is_err());
        }
    }

    #[test]
    fn test_delete_page_deletes_all_content_streams() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let page_index = doc.page_count() as u32;
        doc.add_page(page_index).unwrap();
        doc.append_page_contents(page_index, b"q\nQ\n".to_vec())
            .unwrap();

        let page_ref = doc
            .catalog()
            .get_page(page_index, doc.ctx())
            .unwrap()
            .unwrap()
            .into_reference()
            .unwrap();
        let page_id = page_ref.id();
        let page_dict = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
        let old_stream_ids: Vec<_> = page_dict
            .get("Contents")
            .and_then(PdfObject::as_array)
            .unwrap()
            .into_iter()
            .filter_map(PdfObject::as_reference)
            .map(|reference| reference.id())
            .collect();

        doc.delete_page(page_index).unwrap();

        for object_id in old_stream_ids {
            assert!(doc.resolve_object(object_id).is_err());
        }
    }
}

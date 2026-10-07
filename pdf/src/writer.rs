use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
mod feature_versions;

use crate::document::PdfDocument;
use crate::error::{PdfError, PdfResult};
use crate::objects::{
    PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
    pdf_stream::PdfStream, pdf_string::PdfString,
};

pub fn save_document<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
) -> PdfResult<Vec<u8>> {
    document.save_to_bytes()
}

pub(crate) fn write_document<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
) -> PdfResult<Vec<u8>> {
    write_document_with_options(
        document,
        crate::document::save_options::SaveOptions::default(),
    )
}

fn write_document_with_options<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    options: crate::document::save_options::SaveOptions,
) -> PdfResult<Vec<u8>> {
    // Validate the latest Catalog override as well as the source Header.
    let effective = document.effective_version()?;
    let version = match options.version {
        Some(version) => {
            if version < effective {
                return Err(PdfError::UnsupportedFeature(
                    "PDF version downgrade requires feature conversion".into(),
                ));
            }
            version.as_str()
        }
        None => document.version(),
    };
    if matches!(
        options.compression,
        crate::document::save_options::StreamCompression::FlateUnencoded
    ) && effective.max(options.version.unwrap_or(effective))
        < crate::document::save_options::PdfVersion::V1_2
    {
        return Err(PdfError::UnsupportedFeature(
            "Flate output requires PDF 1.2 or later; select an explicit version upgrade".into(),
        ));
    }
    if document
        .ctx()
        .trailer()
        .get("Encrypt")
        .is_some_and(|obj| !matches!(obj, PdfObject::PdfNull))
    {
        return Err(PdfError::UnsupportedFeature(
            "saving encrypted documents requires a document security handler".into(),
        ));
    }
    validate_serialization_nodes(
        document
            .ctx()
            .trailer()
            .into_iter()
            .map(|(_, value)| (value, 2)),
        1,
        document.ctx().limits(),
    )?;
    document.ctx().materialize_all_objects()?;

    let mut output = Vec::new();
    append_output(
        &mut output,
        format!("%PDF-{version}\n").as_bytes(),
        options.max_output_bytes,
    )?;
    append_output(
        &mut output,
        b"%\xE2\xE3\xCF\xD3\n",
        options.max_output_bytes,
    )?;

    let object_ids = document.ctx().serializable_object_ids();
    let max_obj_num = object_ids.iter().map(|id| id.obj_num()).max().unwrap_or(0);
    let size = max_obj_num
        .checked_add(1)
        .ok_or_else(|| PdfError::ResourceLimit("xref output Size overflow".into()))?;
    let mut xref_entries = allocate_xref_entries(size, document.ctx().limits().max_objects)?;
    if !xref_entries.is_empty() {
        xref_entries[0] = (0, 65535, b'f');
    }

    for object_id in object_ids {
        if let Some(obj) = document.ctx().object_for_write(object_id)? {
            validate_serialization_graph(&obj, document.ctx().limits())?;
            let obj = prepare_document_output_object(document, obj, options.compression)?;
            validate_serialization_graph(&obj, document.ctx().limits())?;
            feature_versions::validate_feature_versions(
                document,
                &obj,
                effective.max(options.version.unwrap_or(effective)),
            )?;
            xref_entries[object_id.obj_num() as usize] =
                (output.len() as u64, object_id.gen_num(), b'n');
            append_output(
                &mut output,
                format!("{} {} obj\n", object_id.obj_num(), object_id.gen_num()).as_bytes(),
                options.max_output_bytes,
            )?;
            write_object_limited(&mut output, &obj, options.max_output_bytes)?;
            append_output(&mut output, b"\nendobj\n", options.max_output_bytes)?;
        }
    }

    let xref_offset = output.len() as u64;
    write_xref_table(&mut output, &xref_entries, options.max_output_bytes)?;
    write_trailer_limited(
        &mut output,
        &document.ctx().trailer(),
        size,
        document.ctx().limits(),
        options.max_output_bytes,
    )?;
    append_output(&mut output, b"startxref\n", options.max_output_bytes)?;
    append_output(
        &mut output,
        format!("{xref_offset}\n").as_bytes(),
        options.max_output_bytes,
    )?;
    append_output(&mut output, b"%%EOF\n", options.max_output_bytes)?;
    Ok(output)
}

fn append_output(output: &mut Vec<u8>, bytes: &[u8], limit: usize) -> PdfResult<()> {
    crate::filter::check_output_size(output.len(), bytes.len(), limit, "PDF output bytes")?;
    output
        .try_reserve_exact(bytes.len())
        .map_err(|_| PdfError::ResourceLimit("PDF output allocation".into()))?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
fn prepare_output_object(
    object: PdfObject,
    compression: crate::document::save_options::StreamCompression,
) -> PdfResult<PdfObject> {
    prepare_output_object_with_limits(
        object,
        compression,
        crate::parser::options::Limits::default(),
    )
}

struct BoundedCompressionOutput {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedCompressionOutput {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(input.len())
            .is_none_or(|size| size > self.limit)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "compressed stream byte limit",
            ));
        }
        self.bytes.try_reserve_exact(input.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "compressed stream allocation",
            )
        })?;
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn compression_error(error: std::io::Error) -> PdfError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::FileTooLarge | std::io::ErrorKind::OutOfMemory
    ) {
        PdfError::ResourceLimit("compressed output stream bytes or allocation".into())
    } else {
        PdfError::IOError(format!("compress output stream failed: {error}"))
    }
}

fn prepare_document_output_object<R: std::io::Read + std::io::Seek>(
    document: &crate::document::PdfDocument<R>,
    object: PdfObject,
    compression: crate::document::save_options::StreamCompression,
) -> PdfResult<PdfObject> {
    use crate::{
        document::save_options::StreamCompression,
        objects::pdf_stream::{PdfStream, resolve_setting_reference},
    };
    if !matches!(compression, StreamCompression::FlateUnencoded) {
        return Ok(object);
    }
    let PdfObject::PdfStream(stream) = object else {
        return Ok(object);
    };
    if ["F", "FFilter", "FDecodeParms"]
        .iter()
        .any(|key| stream.dict().get(key).is_some())
    {
        return Ok(PdfObject::PdfStream(stream));
    }
    let unencoded = match stream.dict().get("Filter") {
        None => true,
        Some(filter) => matches!(
            resolve_setting_reference(filter, document.ctx())?,
            PdfObject::PdfNull
        ),
    };
    if !unencoded {
        return Ok(PdfObject::PdfStream(stream));
    }
    let null_params = stream
        .dict()
        .get("DecodeParms")
        .map(|params| resolve_setting_reference(params, document.ctx()))
        .transpose()?
        .is_none_or(|params| matches!(params, PdfObject::PdfNull));
    let (mut dict, data) = stream.into_raw_parts();
    dict.remove("Filter");
    if null_params {
        dict.remove("DecodeParms");
    }
    prepare_output_object_with_limits(
        PdfObject::PdfStream(PdfStream::new(dict, data)),
        compression,
        document.ctx().limits(),
    )
}

fn prepare_output_object_with_limits(
    object: PdfObject,
    compression: crate::document::save_options::StreamCompression,
    limits: crate::parser::options::Limits,
) -> PdfResult<PdfObject> {
    use crate::{document::save_options::StreamCompression, objects::pdf_name::PdfName};
    if !matches!(compression, StreamCompression::FlateUnencoded) {
        return Ok(object);
    }
    let PdfObject::PdfStream(stream) = object else {
        return Ok(object);
    };
    // Inline bytes do not describe the external payload. Preserve its encoding
    // contract rather than rewriting only one side of that relationship.
    if ["F", "FFilter", "FDecodeParms"]
        .iter()
        .any(|key| stream.dict().get(key).is_some())
    {
        return Ok(PdfObject::PdfStream(stream));
    }
    if stream
        .dict()
        .get("Filter")
        .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
    {
        return Ok(PdfObject::PdfStream(stream));
    }
    if stream
        .dict()
        .get("DecodeParms")
        .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
    {
        return Err(PdfError::UnsupportedFeature(
            "cannot compress unfiltered stream with DecodeParms".into(),
        ));
    }
    crate::filter::check_output_size(
        0,
        stream.raw_data().len(),
        limits.max_raw_stream_bytes.min(limits.max_decoded_bytes),
        "unencoded compression input bytes",
    )?;
    let mut encoder = flate2::write::ZlibEncoder::new(
        BoundedCompressionOutput {
            bytes: Vec::new(),
            limit: limits.max_raw_stream_bytes,
        },
        flate2::Compression::default(),
    );
    encoder
        .write_all(stream.raw_data())
        .map_err(compression_error)?;
    let bytes = encoder.finish().map_err(compression_error)?.bytes;
    let mut dict = stream.dict().clone();
    dict.remove("DecodeParms");
    dict.try_insert(
        "Filter".into(),
        PdfObject::PdfName(PdfName::new("FlateDecode".into())),
    )?;
    Ok(PdfObject::PdfStream(PdfStream::new(dict, bytes)))
}

fn validate_serialization_graph(
    root: &PdfObject,
    limits: crate::parser::options::Limits,
) -> PdfResult<()> {
    validate_serialization_nodes(std::iter::once((root, 1)), 0, limits)
}

fn validate_serialization_nodes<'a>(
    roots: impl Iterator<Item = (&'a PdfObject, usize)>,
    mut nodes: usize,
    limits: crate::parser::options::Limits,
) -> PdfResult<()> {
    crate::filter::check_output_size(0, nodes, limits.max_objects, "serialized object nodes")?;
    if nodes > 0 && limits.max_depth == 0 {
        return Err(PdfError::ResourceLimit("serialized object depth".into()));
    }
    let mut pending = Vec::new();
    for root in roots {
        let used = nodes
            .checked_add(pending.len())
            .ok_or_else(|| PdfError::ResourceLimit("serialized nodes overflow".into()))?;
        crate::filter::check_output_size(used, 1, limits.max_objects, "serialized object nodes")?;
        pending
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("serialization validation allocation".into()))?;
        pending.push(root);
    }
    while let Some((object, depth)) = pending.pop() {
        crate::filter::check_output_size(nodes, 1, limits.max_objects, "serialized object nodes")?;
        nodes += 1;
        if depth > limits.max_depth {
            return Err(PdfError::ResourceLimit("serialized object depth".into()));
        }
        let next_depth = depth
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("serialized depth overflow".into()))?;
        match object {
            PdfObject::PdfArray(array) => {
                crate::filter::check_output_size(
                    nodes + pending.len(),
                    array.len(),
                    limits.max_objects,
                    "serialized array nodes",
                )?;
                pending.try_reserve(array.len()).map_err(|_| {
                    PdfError::ResourceLimit("serialization validation allocation".into())
                })?;
                pending.extend(array.into_iter().map(|child| (child, next_depth)));
            }
            PdfObject::PdfDict(dict) => {
                crate::filter::check_output_size(
                    nodes + pending.len(),
                    dict.into_iter().count(),
                    limits.max_objects,
                    "serialized dictionary nodes",
                )?;
                for (_, child) in dict {
                    pending.try_reserve(1).map_err(|_| {
                        PdfError::ResourceLimit("serialization validation allocation".into())
                    })?;
                    pending.push((child, next_depth));
                }
            }
            PdfObject::PdfStream(stream) => {
                if depth != 1 {
                    return Err(PdfError::ObjectError(
                        "stream must be an indirect object; use an object reference inside containers".into(),
                    ));
                }
                crate::filter::check_output_size(
                    nodes + pending.len(),
                    stream.dict().into_iter().count(),
                    limits.max_objects,
                    "serialized stream dictionary nodes",
                )?;
                for (_, child) in stream.dict() {
                    pending.try_reserve(1).map_err(|_| {
                        PdfError::ResourceLimit("serialization validation allocation".into())
                    })?;
                    pending.push((child, next_depth));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn allocate_xref_entries(size: u32, limit: usize) -> PdfResult<Vec<(u64, u32, u8)>> {
    let size = usize::try_from(size)
        .map_err(|_| PdfError::ResourceLimit("xref output size overflow".into()))?;
    crate::filter::check_output_size(0, size, limit, "xref output entries")?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(size)
        .map_err(|_| PdfError::ResourceLimit("xref output allocation".into()))?;
    entries.resize(size, (0, 0, b'f'));
    Ok(entries)
}

fn write_indirect_object(
    output: &mut Vec<u8>,
    object_id: ObjectId,
    obj: &PdfObject,
) -> PdfResult<()> {
    output.extend_from_slice(
        format!("{} {} obj\n", object_id.obj_num(), object_id.gen_num()).as_bytes(),
    );
    write_object(output, obj)?;
    output.extend_from_slice(b"\nendobj\n");
    Ok(())
}

fn write_xref_table(
    output: &mut Vec<u8>,
    entries: &[(u64, u32, u8)],
    limit: usize,
) -> PdfResult<()> {
    if entries.iter().any(|(offset, generation, state)| {
        *offset > 9_999_999_999 || *generation > 65535 || !matches!(state, b'n' | b'f')
    }) {
        return Err(PdfError::ObjectError(
            "classic xref entry exceeds field bounds or has invalid state".into(),
        ));
    }
    append_output(output, b"xref\n", limit)?;
    append_output(output, format!("0 {}\n", entries.len()).as_bytes(), limit)?;
    for (offset, gen_num, state) in entries {
        append_output(
            output,
            format!("{offset:010} {gen_num:05} {} \n", *state as char).as_bytes(),
            limit,
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn write_trailer(
    output: &mut Vec<u8>,
    original: &PdfDict,
    size: u32,
    limits: crate::parser::options::Limits,
) -> PdfResult<()> {
    write_trailer_limited(output, original, size, limits, usize::MAX)
}

fn write_trailer_limited(
    output: &mut Vec<u8>,
    original: &PdfDict,
    size: u32,
    limits: crate::parser::options::Limits,
    output_limit: usize,
) -> PdfResult<()> {
    let mut trailer = PdfDict::default();
    for (key, value) in original {
        if matches!(
            key.as_str(),
            "Prev" | "XRefStm" | "Type" | "W" | "Index" | "Filter" | "DecodeParms" | "Length"
        ) {
            continue;
        }
        trailer.try_insert(key.clone(), value.clone())?;
    }
    trailer.try_insert(
        "Size".to_string(),
        PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new_from_lexeme(
            size.to_string().as_bytes(),
        )?),
    )?;
    validate_serialization_nodes(trailer.into_iter().map(|(_, value)| (value, 2)), 1, limits)?;
    append_output(output, b"trailer\n", output_limit)?;
    write_dict_limited(output, &trailer, output_limit)?;
    append_output(output, b"\n", output_limit)?;
    Ok(())
}

fn write_object(output: &mut Vec<u8>, obj: &PdfObject) -> PdfResult<()> {
    match obj {
        PdfObject::PdfNull => output.extend_from_slice(b"null"),
        PdfObject::PdfNumber(number) => output.extend_from_slice(format_number(number)?.as_bytes()),
        PdfObject::PdfBool(value) => {
            output.extend_from_slice(if value.value() { b"true" } else { b"false" })
        }
        PdfObject::PdfName(name) => write_name(output, name)?,
        PdfObject::PdfString(string) => write_string(output, string)?,
        PdfObject::PdfArray(array) => write_array(output, array)?,
        PdfObject::PdfDict(dict) => write_dict(output, dict)?,
        PdfObject::PdfStream(stream) => write_stream(output, stream)?,
        PdfObject::PdfReference(reference) => output.extend_from_slice(
            format!("{} {} R", reference.objnum(), reference.gennum()).as_bytes(),
        ),
    }
    Ok(())
}

fn write_object_limited(output: &mut Vec<u8>, object: &PdfObject, limit: usize) -> PdfResult<()> {
    match object {
        PdfObject::PdfArray(array) => {
            append_output(output, b"[", limit)?;
            for (index, item) in array.into_iter().enumerate() {
                if index > 0 {
                    append_output(output, b" ", limit)?;
                }
                write_object_limited(output, item, limit)?;
            }
            append_output(output, b"]", limit)
        }
        PdfObject::PdfDict(dict) => write_dict_limited(output, dict, limit),
        PdfObject::PdfStream(stream) => {
            let mut dict = stream.dict().clone();
            dict.try_insert(
                "Length".into(),
                PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new_from_lexeme(
                    stream.raw_data().len().to_string().as_bytes(),
                )?),
            )?;
            write_dict_limited(output, &dict, limit)?;
            append_output(output, b"\nstream\n", limit)?;
            append_output(output, stream.raw_data(), limit)?;
            append_output(output, b"\nendstream", limit)
        }
        _ => {
            let token_length = match object {
                PdfObject::PdfName(name) => serialized_name_length(name)?,
                PdfObject::PdfString(string) => serialized_string_length(string)?,
                PdfObject::PdfNumber(number) => number.lexeme().len(),
                PdfObject::PdfNull => 4,
                PdfObject::PdfBool(value) => {
                    if value.value() {
                        4
                    } else {
                        5
                    }
                }
                PdfObject::PdfReference(reference) => {
                    format!("{} {} R", reference.objnum(), reference.gennum()).len()
                }
                _ => {
                    return Err(PdfError::ObjectError(
                        "unexpected container in token serialization".into(),
                    ));
                }
            };
            crate::filter::check_output_size(
                output.len(),
                token_length,
                limit,
                "PDF output token bytes",
            )?;
            let mut token = Vec::new();
            write_object(&mut token, object)?;
            append_output(output, &token, limit)
        }
    }
}

fn write_dict_limited(output: &mut Vec<u8>, dict: &PdfDict, limit: usize) -> PdfResult<()> {
    append_output(output, b"<<", limit)?;
    for (key, value) in dict {
        append_output(output, b"\n", limit)?;
        write_object_limited(
            output,
            &PdfObject::PdfName(PdfName::new(key.clone())),
            limit,
        )?;
        append_output(output, b" ", limit)?;
        write_object_limited(output, value, limit)?;
    }
    append_output(output, b"\n>>", limit)
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
        write_name(output, &PdfName::new(key.clone()))?;
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
        PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new_from_lexeme(
            stream.raw_data().len().to_string().as_bytes(),
        )?),
    );
    write_dict(output, &dict)?;
    output.extend_from_slice(b"\nstream\n");
    output.extend_from_slice(stream.raw_data());
    output.extend_from_slice(b"\nendstream");
    Ok(())
}

fn serialized_name_length(name: &PdfName) -> PdfResult<usize> {
    if name.bytes().contains(&0) {
        return Err(PdfError::ObjectError("name contains zero byte".into()));
    }
    name.bytes().iter().try_fold(1usize, |length, byte| {
        let escaped = !(33..=126).contains(byte) || b"()<>[]{}/%#".contains(byte);
        length
            .checked_add(if escaped { 3 } else { 1 })
            .ok_or_else(|| PdfError::ResourceLimit("serialized name size overflow".into()))
    })
}

fn serialized_string_length(string: &PdfString) -> PdfResult<usize> {
    string.bytes().iter().try_fold(2usize, |length, byte| {
        let escaped = matches!(byte, b'(' | b')' | b'\\' | b'\n' | b'\r' | b'\t' | 8 | 12);
        length
            .checked_add(if escaped { 2 } else { 1 })
            .ok_or_else(|| PdfError::ResourceLimit("serialized string size overflow".into()))
    })
}

pub(crate) fn write_name(output: &mut Vec<u8>, name: &PdfName) -> PdfResult<()> {
    let length = serialized_name_length(name)?;
    let escaped = |byte: u8| !(33..=126).contains(&byte) || b"()<>[]{}/%#".contains(&byte);
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("serialized name allocation".into()))?;
    output.extend_from_slice(b"/");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &byte in name.bytes() {
        if escaped(byte) {
            output.extend_from_slice(&[
                b'#',
                HEX[usize::from(byte >> 4)],
                HEX[usize::from(byte & 15)],
            ]);
        } else {
            output.push(byte);
        }
    }
    Ok(())
}

fn write_string(output: &mut Vec<u8>, string: &PdfString) -> PdfResult<()> {
    let length = serialized_string_length(string)?;
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("serialized string allocation".into()))?;
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
    Ok(())
}

fn format_number(number: &crate::objects::pdf_number::PdfNumber) -> PdfResult<String> {
    let lexeme = number.lexeme();
    let unsigned = lexeme.strip_prefix(['+', '-']).unwrap_or(lexeme);
    if unsigned.is_empty()
        || !unsigned.bytes().any(|byte| byte.is_ascii_digit())
        || unsigned
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && byte != b'.')
        || unsigned.bytes().filter(|byte| *byte == b'.').count() > 1
    {
        return Err(PdfError::ObjectError(format!(
            "invalid PDF number: {lexeme}"
        )));
    }
    Ok(lexeme.to_string())
}

impl<R: std::io::Read + std::io::Seek> PdfDocument<R> {
    pub fn save_to_bytes(&self) -> PdfResult<Vec<u8>> {
        write_document(self)
    }

    pub fn save_to_bytes_with_options(
        &self,
        options: crate::document::save_options::SaveOptions,
    ) -> PdfResult<Vec<u8>> {
        write_document_with_options(self, options)
    }

    pub fn save_to_writer_with_options<W: Write>(
        &self,
        output: &mut W,
        options: crate::document::save_options::SaveOptions,
    ) -> PdfResult<()> {
        let bytes = self.save_to_bytes_with_options(options)?;
        output
            .write_all(&bytes)
            .map_err(|error| PdfError::IOError(format!("write PDF output failed: {error}")))
    }

    pub fn save_to_file_with_options<P: AsRef<Path>>(
        &self,
        path: P,
        options: crate::document::save_options::SaveOptions,
    ) -> PdfResult<()> {
        let bytes = self.save_to_bytes_with_options(options)?;
        save_bytes_atomically(path.as_ref(), &bytes)
    }

    /// Write a full rewrite to an output stream. Serialization finishes before
    /// the first write, but an I/O error may leave partial bytes in the writer.
    /// This method does not flush or close the caller-owned stream.
    pub fn save_to_writer<W: Write>(&self, output: &mut W) -> PdfResult<()> {
        let bytes = self.save_to_bytes()?;
        output
            .write_all(&bytes)
            .map_err(|error| PdfError::IOError(format!("write PDF output failed: {error}")))
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> PdfResult<()> {
        let bytes = self.save_to_bytes()?;
        save_bytes_atomically(path.as_ref(), &bytes)
    }
}

struct TemporaryOutput(std::path::PathBuf, bool);
impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.1 {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

fn save_bytes_atomically(path: &Path, bytes: &[u8]) -> PdfResult<()> {
    save_file_atomically(path, |file| file.write_all(bytes))
}

fn save_file_atomically(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> PdfResult<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if path.file_name().is_none() {
        return Err(PdfError::IOError("output path must name a file".into()));
    }
    let permissions = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Some(metadata.permissions()),
        Ok(_) => {
            return Err(PdfError::IOError(
                "output target must be a regular file, not a symlink or directory".into(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(PdfError::IOError(format!(
                "inspect output target failed: {error}"
            )));
        }
    };
    let mut allocated = None;
    for _ in 0..128 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".rzpdf-{}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => {
                allocated = Some((file, TemporaryOutput(temporary, false)));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(PdfError::IOError(format!(
                    "create temporary output failed: {error}"
                )));
            }
        }
    }
    let (mut file, mut temporary) = allocated
        .ok_or_else(|| PdfError::IOError("temporary output name attempts exhausted".into()))?;
    let prepared = (|| {
        write(&mut file)
            .map_err(|error| PdfError::IOError(format!("write temporary PDF failed: {error}")))?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions).map_err(|error| {
                PdfError::IOError(format!("set output permissions failed: {error}"))
            })?;
        }
        file.sync_all()
            .map_err(|error| PdfError::IOError(format!("sync temporary PDF failed: {error}")))
    })();
    // Close before cleanup on success and error paths, including Windows.
    drop(file);
    prepared?;
    std::fs::rename(&temporary.0, path)
        .map_err(|error| PdfError::IOError(format!("replace output file failed: {error}")))?;
    temporary.1 = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn trailer_direct_streams_are_rejected_before_writing_without_mutation() {
        use crate::{
            document::PdfDocument,
            objects::{PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_stream::PdfStream},
        };
        for nested in 0..3 {
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let stream = PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"invalid inline stream".to_vec(),
            ));
            let value = match nested {
                0 => stream,
                1 => {
                    let mut array = PdfArray::default();
                    array.add_obj(stream);
                    PdfObject::PdfArray(array)
                }
                _ => {
                    let mut dict = PdfDict::default();
                    dict.insert("Nested".into(), stream);
                    PdfObject::PdfDict(dict)
                }
            };
            let mut trailer = doc.ctx().trailer().clone();
            trailer.insert("Custom".into(), value);
            doc.ctx().replace_trailer(trailer.clone()).unwrap();
            let revision = doc.revision();
            let mut output = b"existing".to_vec();
            assert!(matches!(
                doc.save_to_writer(&mut output),
                Err(crate::error::PdfError::ObjectError(_))
            ));
            assert_eq!(output, b"existing");
            assert_eq!(doc.revision(), revision);
            assert_eq!(*doc.ctx().trailer(), trailer);
        }
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"valid".to_vec(),
            )))
            .unwrap();
        let mut trailer = doc.ctx().trailer().clone();
        trailer.insert(
            "Custom".into(),
            PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::from_id(id)),
        );
        doc.ctx().replace_trailer(trailer).unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(
            reopened
                .resolve_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            b"valid"
        );
        assert_eq!(
            reopened
                .ctx()
                .trailer()
                .get("Custom")
                .unwrap()
                .as_reference()
                .unwrap()
                .id(),
            id
        );
    }

    #[test]
    fn trailer_serialization_respects_remaining_output_budget() {
        use crate::{
            objects::{PdfObject, pdf_dict::PdfDict, pdf_string::PdfString},
            parser::options::Limits,
        };
        let limits = Limits::default();
        let original = PdfDict::default();
        let mut expected = Vec::new();
        super::write_trailer(&mut expected, &original, 1, limits).unwrap();
        let mut exact = Vec::new();
        super::write_trailer_limited(&mut exact, &original, 1, limits, expected.len()).unwrap();
        assert_eq!(exact, expected);
        let mut limited = Vec::new();
        assert!(matches!(
            super::write_trailer_limited(&mut limited, &original, 1, limits, expected.len() - 1),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert!(limited.len() < expected.len());
        let mut original = PdfDict::default();
        original
            .try_insert(
                "Large".into(),
                PdfObject::PdfString(PdfString::new(vec![b'x'; 1024], false)),
            )
            .unwrap();
        let mut limited = Vec::new();
        assert!(super::write_trailer_limited(&mut limited, &original, 1, limits, 100).is_err());
        assert!(limited.len() <= 100);
        assert!(!limited.contains(&b'x'));
    }

    #[test]
    fn token_preflight_uses_exact_escaped_length_before_output() {
        use crate::objects::{PdfObject, pdf_name::PdfName, pdf_string::PdfString};
        let string = PdfObject::PdfString(PdfString::new((0..=255).collect(), false));
        let name = PdfObject::PdfName(PdfName::new("A /#中文".into()));
        for object in [string, name] {
            let mut expected = Vec::new();
            super::write_object(&mut expected, &object).unwrap();
            let mut exact = Vec::new();
            super::write_object_limited(&mut exact, &object, expected.len()).unwrap();
            assert_eq!(exact, expected);
            let mut rejected = b"prefix".to_vec();
            let limit = rejected.len() + expected.len() - 1;
            assert!(matches!(
                super::write_object_limited(&mut rejected, &object, limit),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(rejected, b"prefix");
        }
    }

    #[test]
    fn limited_serialization_bounds_nested_containers_and_stream_copy() {
        use crate::objects::{
            PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_stream::PdfStream,
        };
        let mut array = PdfArray::default();
        for _ in 0..20 {
            array.try_add_obj(PdfObject::PdfNull).unwrap();
        }
        let object = PdfObject::PdfArray(array);
        let mut output = Vec::new();
        assert!(matches!(
            super::write_object_limited(&mut output, &object, 10),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert!(output.len() <= 10);
        let mut expected = Vec::new();
        super::write_object(&mut expected, &object).unwrap();
        let mut exact = Vec::new();
        super::write_object_limited(&mut exact, &object, expected.len()).unwrap();
        assert_eq!(exact, expected);
        let stream = PdfObject::PdfStream(PdfStream::new(PdfDict::default(), vec![b'x'; 1024]));
        let mut output = Vec::new();
        assert!(super::write_object_limited(&mut output, &stream, 100).is_err());
        assert!(output.len() <= 100);
        assert!(!output.contains(&b'x'));
    }

    #[test]
    fn xref_block_checks_budget_per_line_and_classic_field_bounds() {
        let entries = [(0, 65535, b'f'), (9_999_999_999, 0, b'n')];
        let mut exact = Vec::new();
        super::write_xref_table(&mut exact, &entries, usize::MAX).unwrap();
        let size = exact.len();
        let mut repeated = Vec::new();
        super::write_xref_table(&mut repeated, &entries, size).unwrap();
        assert_eq!(repeated, exact);
        let mut limited = Vec::new();
        assert!(matches!(
            super::write_xref_table(&mut limited, &entries, size - 1),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert!(limited.len() <= size - 1);
        for entry in [(10_000_000_000, 0, b'n'), (0, 65536, b'f'), (0, 0, b'x')] {
            let mut output = b"existing".to_vec();
            assert!(super::write_xref_table(&mut output, &[entry], usize::MAX).is_err());
            assert_eq!(output, b"existing");
        }
    }

    #[test]
    fn compression_budget_bounds_finish_bytes_and_input() {
        use crate::{
            document::save_options::StreamCompression,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::options::Limits,
        };
        let empty = PdfObject::PdfStream(PdfStream::new(PdfDict::default(), Vec::new()));
        let compressed =
            super::prepare_output_object(empty.clone(), StreamCompression::FlateUnencoded).unwrap();
        let length = compressed.as_stream().unwrap().raw_data().len();
        let limits = Limits {
            max_raw_stream_bytes: length,
            ..Default::default()
        };
        assert!(
            super::prepare_output_object_with_limits(
                empty.clone(),
                StreamCompression::FlateUnencoded,
                limits
            )
            .is_ok()
        );
        assert!(matches!(
            super::prepare_output_object_with_limits(
                empty,
                StreamCompression::FlateUnencoded,
                Limits {
                    max_raw_stream_bytes: length - 1,
                    ..limits
                }
            ),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        let input = PdfObject::PdfStream(PdfStream::new(PdfDict::default(), vec![0; 4]));
        assert!(matches!(
            super::prepare_output_object_with_limits(
                input,
                StreamCompression::FlateUnencoded,
                Limits {
                    max_decoded_bytes: 3,
                    ..Default::default()
                }
            ),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn bounded_compression_sink_rejects_growth_without_partial_append() {
        use std::io::Write;
        let mut sink = super::BoundedCompressionOutput {
            bytes: Vec::new(),
            limit: 3,
        };
        sink.write_all(b"ab").unwrap();
        assert_eq!(
            sink.write_all(b"cd").unwrap_err().kind(),
            std::io::ErrorKind::FileTooLarge
        );
        assert_eq!(sink.bytes, b"ab");
        sink.write_all(b"c").unwrap();
        assert_eq!(sink.bytes, b"abc");
    }

    #[test]
    fn output_compression_preserves_encoded_streams_and_rejects_ambiguous_parameters() {
        use crate::{
            document::save_options::StreamCompression,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream},
        };
        let mut dict = PdfDict::default();
        dict.try_insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("DCTDecode".into())),
        )
        .unwrap();
        let object = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
        assert_eq!(
            super::prepare_output_object(object.clone(), StreamCompression::FlateUnencoded)
                .unwrap(),
            object
        );
        let mut dict = PdfDict::default();
        dict.try_insert("DecodeParms".into(), PdfObject::PdfDict(PdfDict::default()))
            .unwrap();
        assert!(matches!(
            super::prepare_output_object(
                PdfObject::PdfStream(PdfStream::new(dict, vec![1])),
                StreamCompression::FlateUnencoded
            ),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
        let mut dict = PdfDict::default();
        dict.try_insert("Filter".into(), PdfObject::PdfNull)
            .unwrap();
        dict.try_insert("DecodeParms".into(), PdfObject::PdfNull)
            .unwrap();
        let output = super::prepare_output_object(
            PdfObject::PdfStream(PdfStream::new(dict, Vec::new())),
            StreamCompression::FlateUnencoded,
        )
        .unwrap();
        let dict = output.as_stream().unwrap().dict();
        assert!(dict.get("DecodeParms").is_none());
        assert_eq!(
            dict.get("Filter").unwrap().as_name().unwrap().bytes(),
            b"FlateDecode"
        );
    }

    #[test]
    fn temporary_partial_write_failure_preserves_target_and_cleans_output() {
        use std::io::Write;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("rzpdf-write-fault-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let target = directory.join("target.pdf");
        std::fs::write(&target, b"original bytes").unwrap();
        let result = super::save_file_atomically(&target, |file| {
            file.write_all(b"partial")?;
            Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "injected full disk",
            ))
        });
        assert!(matches!(result, Err(crate::error::PdfError::IOError(_))));
        assert_eq!(std::fs::read(&target).unwrap(), b"original bytes");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn final_trailer_counts_generated_size_before_output() {
        use crate::{error::PdfError, objects::pdf_dict::PdfDict, parser::options::Limits};
        let original = PdfDict::default();
        let limits = Limits {
            max_objects: 2,
            max_depth: 2,
            ..Default::default()
        };
        for rejected in [
            Limits {
                max_objects: 1,
                ..limits
            },
            Limits {
                max_depth: 1,
                ..limits
            },
        ] {
            let mut output = b"existing".to_vec();
            assert!(matches!(
                super::write_trailer(&mut output, &original, 1, rejected),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(output, b"existing");
        }
        let mut output = Vec::new();
        super::write_trailer(&mut output, &original, 1, limits).unwrap();
        assert!(output.starts_with(b"trailer\n"));
        assert!(output.windows(7).any(|part| part == b"/Size 1"));
    }

    #[test]
    fn serialization_preflight_bounds_direct_nesting_and_nodes() {
        use crate::{
            objects::{PdfObject, pdf_array::PdfArray},
            parser::options::Limits,
        };
        let mut object = PdfObject::PdfNull;
        for _ in 0..3 {
            let mut array = PdfArray::default();
            array.add_obj(object);
            object = PdfObject::PdfArray(array);
        }
        let limits = Limits {
            max_depth: 4,
            max_objects: 4,
            ..Default::default()
        };
        assert!(super::validate_serialization_graph(&object, limits).is_ok());
        assert!(
            super::validate_serialization_nodes(
                std::iter::once((&object, 2)),
                1,
                Limits {
                    max_depth: 5,
                    max_objects: 5,
                    ..limits
                }
            )
            .is_ok()
        );
        assert!(
            super::validate_serialization_nodes(
                std::iter::once((&object, 2)),
                1,
                Limits {
                    max_depth: 4,
                    max_objects: 5,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            super::validate_serialization_graph(
                &object,
                Limits {
                    max_depth: 3,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            super::validate_serialization_graph(
                &object,
                Limits {
                    max_objects: 3,
                    ..limits
                }
            )
            .is_err()
        );
    }
    #[test]
    fn xref_output_checks_sparse_slot_count_before_allocation() {
        assert_eq!(
            super::allocate_xref_entries(4, 4).unwrap(),
            vec![(0, 0, b'f'); 4]
        );
        assert!(super::allocate_xref_entries(0, 0).unwrap().is_empty());
        assert!(matches!(
            super::allocate_xref_entries(5, 4),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert!(matches!(
            super::allocate_xref_entries(u32::MAX, 4),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
    }
    #[test]
    fn serialized_string_roundtrips_all_byte_values() {
        use crate::{
            io::stream_reader::StreamReader, objects::pdf_string::PdfString,
            parser::syntax::SyntaxParser,
        };
        let bytes: Vec<u8> = (0..=255).collect();
        let mut output = Vec::new();
        super::write_string(&mut output, &PdfString::new(bytes.clone(), false)).unwrap();
        let reader = StreamReader::try_new(std::io::Cursor::new(output)).unwrap();
        let object = SyntaxParser::new(reader).get_object().unwrap();
        assert_eq!(object.as_string().unwrap().bytes(), bytes);
        let mut output = Vec::new();
        super::write_string(&mut output, &PdfString::new(Vec::new(), true)).unwrap();
        assert_eq!(output, b"()");
    }
    #[test]
    fn number_serialization_preserves_large_integer_and_rejects_nonfinite() {
        use crate::objects::pdf_number::PdfNumber;
        let number = PdfNumber::new_from_lexeme(b"9007199254740993").unwrap();
        assert_eq!(super::format_number(&number).unwrap(), "9007199254740993");
        assert_eq!(
            super::format_number(&PdfNumber::new_from_lexeme(b"-.125").unwrap()).unwrap(),
            "-.125"
        );
        assert!(super::format_number(&PdfNumber::new(f32::NAN)).is_err());
        assert!(super::format_number(&PdfNumber::new(f32::INFINITY)).is_err());
    }

    #[test]
    fn names_escape_delimiters_whitespace_and_nonascii() {
        let mut output = Vec::new();
        super::write_name(
            &mut output,
            &crate::objects::pdf_name::PdfName::new("A B/#é".into()),
        )
        .unwrap();
        assert_eq!(output, b"/A#20B#2F#23#C3#A9");
        output.clear();
        super::write_name(&mut output, &PdfName::new_from_buffer(b"/#ff#80").unwrap()).unwrap();
        assert_eq!(output, b"/#FF#80");
        output.clear();
        assert!(super::write_name(&mut output, &PdfName::new("A\0B".into())).is_err());
        assert!(output.is_empty());
    }

    use crate::{
        document::PdfDocument,
        objects::{
            PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
            pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
        },
        page::content_builder::ContentBuilder,
    };

    fn nest_root_page_tree<R: std::io::Read + std::io::Seek>(doc: &PdfDocument<R>) {
        let trailer = doc.ctx().trailer();
        let root_ref = trailer
            .get("Root")
            .and_then(PdfObject::as_reference)
            .unwrap();
        let root_dict = doc
            .resolve_object(root_ref.id())
            .unwrap()
            .into_dict()
            .unwrap();
        let root_pages_ref = root_dict
            .get("Pages")
            .cloned()
            .and_then(PdfObject::into_reference)
            .unwrap();
        let root_pages_id = root_pages_ref.id();
        let mut root_pages = doc
            .resolve_object(root_pages_id)
            .unwrap()
            .into_dict()
            .unwrap();
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

        let mut first_page = doc
            .resolve_object(first_page_id)
            .unwrap()
            .into_dict()
            .unwrap();
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
    fn mixed_depth_page_order_matches_edit_order_and_cycles_fail() {
        use crate::document::page_tree::PageTree;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        nest_root_page_tree(&doc);
        let first = PageTree::entries(doc.ctx()).unwrap()[0].page_id;
        let original = doc.resolve_object(first).unwrap();
        let mut second = original.clone().into_dict().unwrap();
        let (root_id, mut root) = {
            let catalog = doc.ctx().get_root_owned().unwrap();
            let id = catalog.get("Pages").unwrap().as_reference().unwrap().id();
            (id, doc.resolve_object(id).unwrap().into_dict().unwrap())
        };
        second.insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(root_id)),
        );
        let second_id = doc.insert_object(PdfObject::PdfDict(second));
        let mut kids = root.get("Kids").unwrap().as_array().unwrap().clone();
        kids.add_obj(PdfObject::PdfReference(PdfReference::from_id(second_id)));
        root.insert("Kids".into(), PdfObject::PdfArray(kids));
        root.insert("Count".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        doc.update_object(root_id, PdfObject::PdfDict(root.clone()))
            .unwrap();
        let first_ref = doc.catalog().get_page(0, doc.ctx()).unwrap().unwrap();
        assert_eq!(first_ref.as_reference().unwrap().id(), first);
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        assert_ne!(doc.resolve_object(first).unwrap(), original);
        let mut cycle = PdfArray::default();
        cycle.add_obj(PdfObject::PdfReference(PdfReference::from_id(root_id)));
        root.insert("Kids".into(), PdfObject::PdfArray(cycle));
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        assert!(doc.catalog().total_page(doc.ctx()).is_err());
    }

    #[test]
    fn all_resolution_paths_observe_edits_and_keep_original_snapshot() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let original = doc.ctx().resolve_original_object(root_id).unwrap().clone();
        let replacement = PdfObject::PdfString(PdfString::new(b"replacement".to_vec(), false));
        doc.update_object(root_id, replacement.clone()).unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(root_id));
        assert_eq!(doc.ctx().resolve(&reference).unwrap(), replacement);
        assert_eq!(doc.ctx().resolve_owned(&reference).unwrap(), replacement);
        assert_eq!(doc.ctx().resolve_object_id(root_id).unwrap(), replacement);
        assert_eq!(
            doc.ctx().resolve_original_object(root_id).unwrap(),
            &original
        );
        let inserted = doc.insert_object(replacement.clone());
        assert_eq!(doc.ctx().resolve_object_id(inserted).unwrap(), replacement);
        doc.delete_object(root_id).unwrap();
        assert!(doc.ctx().resolve(&reference).is_err());
        assert!(doc.ctx().resolve_object_id(root_id).is_err());
    }

    #[test]
    fn page_inheritance_reads_latest_ancestor_and_count_is_validated() {
        use crate::document::page_tree::PageTree;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        nest_root_page_tree(&doc);
        let entry = PageTree::entries(doc.ctx()).unwrap().remove(0);
        let mut leaf = entry.page_dict;
        leaf.remove("Resources");
        doc.update_object(entry.page_id, PdfObject::PdfDict(leaf))
            .unwrap();
        let mut parent = doc
            .resolve_object(entry.parent_id)
            .unwrap()
            .into_dict()
            .unwrap();
        let mut resources = PdfDict::default();
        resources.insert("Marker".into(), PdfObject::PdfNumber(PdfNumber::new(42.0)));
        parent.insert("Resources".into(), PdfObject::PdfDict(resources));
        doc.update_object(entry.parent_id, PdfObject::PdfDict(parent.clone()))
            .unwrap();
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .resource()
                .unwrap()
                .get("Marker")
                .unwrap()
                .as_i32(),
            Some(42)
        );
        parent.insert("Count".into(), PdfObject::PdfNumber(PdfNumber::new(9.0)));
        doc.update_object(entry.parent_id, PdfObject::PdfDict(parent))
            .unwrap();
        assert!(doc.try_page_count().is_err());
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
        doc.replace_page_content_builder(page_index, &builder)
            .unwrap();

        let mut page = doc.get_page(page_index).unwrap();
        assert!(page.display_list().is_ok());

        let bytes = doc.save_to_bytes().unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        let mut saved_page = saved.get_page(page_index).unwrap();
        assert!(saved_page.display_list().is_ok());
    }

    #[test]
    fn test_replace_page_contents_preserves_unlinked_streams_until_cleanup() {
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
            assert!(doc.resolve_object(object_id).is_ok());
        }
    }

    #[test]
    fn test_delete_page_preserves_unlinked_streams_until_cleanup() {
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
            assert!(doc.resolve_object(object_id).is_ok());
        }
    }

    #[test]
    fn shared_contents_survive_replacement_deletion_and_save() {
        use crate::document::page_tree::PageTree;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"q Q".to_vec()).unwrap();
        doc.add_page(index + 1).unwrap();
        let entries = PageTree::entries(doc.ctx()).unwrap();
        let shared = entries[index as usize]
            .page_dict
            .get("Contents")
            .unwrap()
            .clone();
        let shared_id = shared.as_reference().unwrap().id();
        let mut second = entries[index as usize + 1].page_dict.clone();
        second.insert("Contents".into(), shared);
        doc.update_object(
            entries[index as usize + 1].page_id,
            PdfObject::PdfDict(second),
        )
        .unwrap();
        doc.replace_page_contents(index, b"q q Q Q".to_vec())
            .unwrap();
        assert_eq!(
            doc.resolve_object(shared_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            b"q Q"
        );
        assert!(doc.get_page(index + 1).unwrap().display_list().is_ok());
        let saved = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert!(saved.get_page(index + 1).unwrap().display_list().is_ok());
        // Re-share the stream and delete one of its owners.
        let mut first = doc
            .resolve_object(entries[index as usize].page_id)
            .unwrap()
            .into_dict()
            .unwrap();
        first.insert(
            "Contents".into(),
            PdfObject::PdfReference(PdfReference::from_id(shared_id)),
        );
        doc.update_object(entries[index as usize].page_id, PdfObject::PdfDict(first))
            .unwrap();
        doc.delete_page(index).unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
        let saved = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(
            saved
                .resolve_object(shared_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            b"q Q"
        );
        assert!(saved.get_page(index).unwrap().display_list().is_ok());
    }

    #[test]
    fn transactions_restore_page_edits_objects_and_allocations() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let before = doc.try_page_count().unwrap();
        let root = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let original = doc.resolve_object(root).unwrap();
        let mut failed_id = None;
        let result: crate::error::PdfResult<()> = doc.edit_transaction(|doc| {
            failed_id = Some(doc.insert_object(PdfObject::PdfNull));
            doc.add_page(before as u32)?;
            doc.replace_page_contents(0, b"q Q".to_vec())?;
            doc.delete_object(root)?;
            Err(crate::error::PdfError::DocumentError(
                "forced failure".into(),
            ))
        });
        assert!(result.is_err());
        assert_eq!(doc.try_page_count().unwrap(), before);
        assert_eq!(doc.resolve_object(root).unwrap(), original);
        assert!(doc.resolve_object(failed_id.unwrap()).is_err());
        assert_eq!(doc.insert_object(PdfObject::PdfNull), failed_id.unwrap());
        let saved = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(saved.try_page_count().unwrap(), before);
    }

    #[test]
    fn transaction_savepoints_and_unwinding_restore_changes() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let before = doc.try_page_count().unwrap();
        doc.edit_transaction(|doc| {
            doc.add_page(before as u32)?;
            let nested: crate::error::PdfResult<()> = doc.edit_transaction(|doc| {
                doc.delete_page(0)?;
                Err(crate::error::PdfError::DocumentError(
                    "savepoint failure".into(),
                ))
            });
            assert!(nested.is_err());
            assert_eq!(doc.try_page_count()?, before + 1);
            Ok(())
        })
        .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: crate::error::PdfResult<()> = doc.edit_transaction(|doc| {
                doc.delete_page(0)?;
                panic!("forced unwind");
            });
        }));
        assert!(result.is_err());
        assert_eq!(doc.try_page_count().unwrap(), before + 1);
    }

    #[test]
    fn save_version_policy_uses_latest_effective_catalog_version() {
        use crate::document::save_options::{PdfVersion, SaveOptions};
        use crate::error::PdfError;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let version_id = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("1.7".into())))
            .unwrap();
        let mut root = doc.ctx().get_root_owned().unwrap();
        root.try_insert(
            "Version".into(),
            PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::from_id(
                version_id,
            )),
        )
        .unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        doc.update_object(version_id, PdfObject::PdfName(PdfName::new("2.0".into())))
            .unwrap();
        let saved = doc.save_to_bytes().unwrap();
        assert!(saved.starts_with(b"%PDF-1.7"));
        let reopened =
            PdfDocument::from_bytes_with_mode(saved, crate::parser::syntax::ParseMode::Strict)
                .unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V2_0);
        let mut output = b"unchanged".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_options(
                &mut output,
                SaveOptions {
                    version: Some(PdfVersion::V1_7),
                    ..SaveOptions::default()
                }
            ),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(output, b"unchanged");
        let upgraded = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V2_0),
                ..SaveOptions::default()
            })
            .unwrap();
        assert!(upgraded.starts_with(b"%PDF-2.0"));
        assert_eq!(
            PdfDocument::from_bytes(upgraded)
                .unwrap()
                .effective_version()
                .unwrap(),
            PdfVersion::V2_0
        );
    }

    #[test]
    fn malformed_catalog_version_is_rejected_before_writer_output() {
        let doc = PdfDocument::new().unwrap();
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        for value in [
            PdfObject::PdfName(PdfName::new("9.9".into())),
            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(2.0)),
        ] {
            let mut root = doc.ctx().get_root_owned().unwrap();
            root.try_insert("Version".into(), value).unwrap();
            doc.update_object(root_id, PdfObject::PdfDict(root))
                .unwrap();
            let mut output = b"unchanged".to_vec();
            assert!(doc.save_to_writer(&mut output).is_err());
            assert_eq!(output, b"unchanged");
        }
    }

    #[test]
    fn flate_minimum_version_uses_catalog_override_without_changing_header() {
        use crate::document::save_options::{PdfVersion, SaveOptions, StreamCompression};
        use crate::error::PdfError;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let mut bytes = doc.save_to_bytes().unwrap();
        bytes[5..8].copy_from_slice(b"1.0");
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        let options = SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..SaveOptions::default()
        };
        assert!(matches!(
            doc.save_to_bytes_with_options(options),
            Err(PdfError::UnsupportedFeature(_))
        ));
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let mut root = doc.ctx().get_root_owned().unwrap();
        root.try_insert(
            "Version".into(),
            PdfObject::PdfName(PdfName::new("1.4".into())),
        )
        .unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        doc.replace_page_contents(0, b"q 0 0 10 10 re f Q".to_vec())
            .unwrap();
        let saved = doc.save_to_bytes_with_options(options).unwrap();
        assert!(saved.starts_with(b"%PDF-1.0"));
        let reopened =
            PdfDocument::from_bytes_with_mode(saved, crate::parser::syntax::ParseMode::Strict)
                .unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_4);
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            1
        );
    }

    #[test]
    fn saving_preserves_source_version_and_binary_header() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let mut bytes = doc.save_to_bytes().unwrap();
        // Header replacements have the same byte length, preserving xref offsets.
        bytes[5..8].copy_from_slice(b"1.4");
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(doc.version(), "1.4");
        let saved = doc.save_to_bytes().unwrap();
        assert!(saved.starts_with(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n"));
        assert_eq!(PdfDocument::from_bytes(saved).unwrap().version(), "1.4");
    }

    #[test]
    fn encrypted_document_save_is_rejected_before_writing() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let bytes = doc.save_to_bytes().unwrap();
        let trailer_offset = bytes
            .windows(b"trailer\n".len())
            .rposition(|window| window == b"trailer\n")
            .unwrap();
        // Modify only the trailer, leaving indirect-object/xref offsets unchanged.
        let mut encrypted = bytes[..trailer_offset].to_vec();
        let mut trailer = bytes[trailer_offset..].to_vec();
        let position = trailer.windows(2).position(|w| w == b"<<").unwrap() + 2;
        trailer.splice(position..position, b" /Encrypt <<>>".iter().copied());
        encrypted.extend(trailer);
        let encrypted = PdfDocument::from_bytes(encrypted).unwrap();
        assert!(matches!(
            encrypted.save_to_bytes(),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
    }

    #[test]
    fn compatible_content_reports_unknown_and_unsupported_operators() {
        use crate::page::diagnostic::DiagnosticKind;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        let mut state = PdfDict::default();
        state.insert(
            "SMask".into(),
            PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("None".into())),
        );
        let mut states = PdfDict::default();
        states.insert("G".into(), PdfObject::PdfDict(state));
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
        doc.add_page_with_builder(
            index,
            crate::page::page_builder::PageBuilder::new().resources(resources),
        )
        .unwrap();
        doc.replace_page_contents(
            index,
            b"customOp /S sh /RelativeColorimetric ri /G gs 0 0 10 10 re f".to_vec(),
        )
        .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 1);
        let diagnostics = list.diagnostics();
        assert_eq!(diagnostics.len(), 4);
        assert_eq!(diagnostics[0].operator, "customOp");
        assert_eq!(diagnostics[0].byte_offset, Some(0));
        assert_eq!(diagnostics[0].kind, DiagnosticKind::UnknownOperator);
        assert_eq!(diagnostics[1].kind, DiagnosticKind::UnsupportedOperator);
        assert_eq!(diagnostics[1].operator, "sh");
        let saved = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert!(matches!(
            saved.get_page(index).unwrap().display_list(),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
    }

    #[test]
    fn malformed_numeric_content_operands_return_errors() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "/bad 0 m",
            "0 /bad l",
            "1 0 0 /bad 0 0 cm",
            "BT /bad 0 Td",
            "BT 0 /bad TD",
            "BT /bad TL",
            "BT /bad Ts",
            "BT /bad Tz",
            "BT 1 0 0 1 /bad 0 Tm",
            "BT /F /bad Tf",
            "BT /bad 0 (text) \"",
            "300000000000000000000000000000000000000 0 300000000000000000000000000000000000000 1 re S",
            "0 300000000000000000000000000000000000000 1 300000000000000000000000000000000000000 re S",
            "100000000000000000000 0 0 1 0 0 cm 100000000000000000000 0 0 1 0 0 cm",
            "BT 1 0 0 1 300000000000000000000000000000000000000 0 Tm 300000000000000000000000000000000000000 0 Td ET",
            "BT 1 0 0 1 0 -300000000000000000000000000000000000000 Tm 300000000000000000000000000000000000000 TL T* ET",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                doc.get_page(index).unwrap().display_list().is_err(),
                "{source}"
            );
        }
        doc.replace_page_contents(index, b"0 0 m 10 10 l S".to_vec())
            .unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
    }

    #[test]
    fn malformed_typed_content_operands_return_errors() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "1 Do",
            "BT 1 10 Tf",
            "BT 8 Tr",
            "BT -1 Tr",
            "BT 1.5 Tr",
            "1 0 d",
            "[/bad] 0 d",
            "[-1] 0 d",
            "[0 0] 0 d",
            "[1] -1 d",
            "/bad M",
            "0 M",
            "BT",
            "ET",
            "BT BT ET ET",
            "BT ET ET",
            "q",
            "q q Q",
            "Q",
            "0 0 Td",
            "1 0 0 1 0 0 Tm",
            "T*",
            "(text) Tj",
            "[(text)] TJ",
            "/bad J",
            "/bad j",
            "null J",
            "1.5 J",
            "1.0 j",
            "-1 J",
            "3 J",
            "3 j",
            "256 j",
            "-1 w",
            "1 2 l",
            "1 2 3 4 v",
            "1 2 3 4 y",
            "1 2 3 4 5 6 c",
            "0 0 m n 1 2 l",
            "0 0 m S 1 2 3 4 y",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                doc.get_page(index).unwrap().display_list().is_err(),
                "{source}"
            );
        }
        doc.replace_page_contents(
            index,
            b"[] 0 d [2 1] 0 d 1 M 0 J 1 J 2 J 0 j 1 j 2 j 0 w 1.5 w 0 0 m 10 10 l S".to_vec(),
        )
        .unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
    }

    #[test]
    fn text_array_validation_and_adjustments_preserve_vertical_scale() {
        use crate::{
            document::page_tree::PageTree, io::stream_reader::StreamReader,
            page::page_object::PageObject, parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let doc = PdfDocument::open_with_options(
            "../tests/path-test.pdf",
            crate::parser::options::ParseOptions {
                limits: crate::parser::options::Limits {
                    max_text_chars: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in ["BT 1 TJ", "BT [null] TJ", "BT [(text)] TJ", "BT (text) Tj"] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(doc.get_page(index).unwrap().display_list().is_err());
        }
        let id = PageTree::entries(doc.ctx()).unwrap()[index as usize].page_id;
        let mut page = doc
            .ctx()
            .resolve_object_owned(id)
            .unwrap()
            .into_dict()
            .unwrap();
        let resources = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >>"
                    .to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap();
        page.insert("Resources".into(), resources);
        doc.update_object(id, PdfObject::PdfDict(page)).unwrap();
        for source in [
            "BT /F 10 Tf (ABC) Tj ET",
            "BT /F 10 Tf [(A) (BC)] TJ ET",
            "BT /F 10 Tf (AB) Tj (C) Tj ET",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(matches!(
                doc.get_page(index).unwrap().display_list(),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
        }
        doc.replace_page_contents(
            index,
            b"BT /F 10 Tf 1 0 0 1 0 20 Tm [(A) 100 (B)] TJ ET".to_vec(),
        )
        .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Text(text) = &list.objects()[0] else {
            panic!("Expected text");
        };
        assert_eq!(text.items.len(), 2);
        assert_eq!(text.items[0].pos().y(), text.items[1].pos().y());
        assert_ne!(text.items[0].pos().x(), text.items[1].pos().x());
        let previous_y = text.items[0].pos().y();
        doc.replace_page_contents(
            index,
            b"BT /F 10 Tf q 1 0 0 1 10 20 Tm Q (A) Tj ET".to_vec(),
        )
        .unwrap();
        let restored = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Text(text) = &restored.objects()[0] else {
            panic!("Expected text");
        };
        assert_eq!(text.items[0].pos().y(), previous_y);
        assert_eq!(text.items[0].pos().x(), 10.0);
        doc.replace_page_contents(
            index,
            b"BT /F 10 Tf 3 TL 1 0 0 1 0 20 Tm 0 0 (AB) \" ET".to_vec(),
        )
        .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Text(text) = &list.objects()[0] else {
            panic!("Expected text");
        };
        // This unrotated page maps decreasing PDF Y to increasing display Y.
        assert_eq!(text.items[0].pos().y(), previous_y + 3.0);
        assert_eq!(text.items[1].pos().y(), previous_y + 3.0);
        doc.replace_page_contents(
            index,
            b"BT /F 300000000000000000000000000000000000000 Tf (A) Tj ET".to_vec(),
        )
        .unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
        doc.replace_page_contents(index, b"BT /F 300000000000000000000000000000000000000 Tf 300000000000000000000000000000000000000 Tc (A) Tj ET".to_vec()).unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_err());
    }

    #[test]
    fn inline_images_are_explicitly_rejected_until_decoder_is_implemented() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"BI /W 1 /H 1 ID x EI".to_vec())
            .unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
    }

    #[test]
    fn rectangle_paths_are_closed_and_keep_negative_dimensions() {
        use crate::page::page_object::PageObject;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"10 20 -3 -4 re S".to_vec())
            .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("Expected path");
        };
        let subpath = &path.pdf_path().subpaths()[0];
        assert!(subpath.is_closed());
        assert_eq!(subpath.segments().len(), 5);
        let crate::path::pdf_path::Segment::LineTo(point) = &subpath.segments()[2] else {
            panic!("Expected corner");
        };
        assert_eq!((point.x(), point.y()), (7.0, 16.0));
    }

    #[test]
    fn pending_clip_applies_after_painting_for_all_path_terminators() {
        use crate::page::{graphic_state::FillType, page_object::PageObject};
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for paint in ["f", "S", "B", "B*", "b", "b*"] {
            for (clip, expected) in [("W", FillType::Winding), ("W*", FillType::EvenOdd)] {
                let source = format!("0 0 10 10 re {clip} {paint} 20 20 5 5 re S 30 30 2 2 re S");
                doc.replace_page_contents(index, source.into_bytes())
                    .unwrap();
                let list = doc.get_page(index).unwrap().display_list().unwrap();
                assert_eq!(list.objects().len(), 3);
                for (i, object) in list.objects().iter().enumerate() {
                    let PageObject::Path(path) = object else {
                        panic!("Expected path");
                    };
                    let elements = path.graphic_state().clip_path.elements();
                    assert_eq!(elements.len(), usize::from(i > 0), "{paint} {clip}");
                    if i > 0 {
                        assert_eq!(elements[0].fill_type, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn empty_clip_is_preserved_and_graphics_restore_recovers_it() {
        use crate::page::page_object::PageObject;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for end in ["n", "S", "f", "B", "B*", "b", "b*"] {
            let source = format!("q W {end} 0 0 10 10 re S Q 20 20 5 5 re S");
            doc.replace_page_contents(index, source.into_bytes())
                .unwrap();
            let list = doc.get_page(index).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2, "{end}");
            let PageObject::Path(first) = &list.objects()[0] else {
                panic!("Expected path");
            };
            assert_eq!(first.graphic_state().clip_path.elements().len(), 1);
            assert!(
                first.graphic_state().clip_path.elements()[0]
                    .path
                    .is_empty()
            );
            let PageObject::Path(second) = &list.objects()[1] else {
                panic!("Expected path");
            };
            assert!(second.graphic_state().clip_path.is_empty());
        }
    }

    #[test]
    fn path_move_close_and_discard_keep_correct_state() {
        use crate::{page::page_object::PageObject, path::pdf_path::Segment};
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"10 20 m 30 40 l 50 60 m 70 80 l h h S".to_vec())
            .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("Expected path");
        };
        let subpaths = path.pdf_path().subpaths();
        assert_eq!(subpaths.len(), 2);
        assert_eq!(subpaths[0].segments().len(), 2);
        assert_eq!(subpaths[1].segments().len(), 3);
        assert!(matches!(subpaths[1].segments()[2], Segment::Close));
        doc.replace_page_contents(index, b"10 20 m 30 40 l h 50 60 70 80 v S".to_vec())
            .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("Expected path");
        };
        let Segment::CurveTo(curve) = &path.pdf_path().subpaths()[0].segments()[3] else {
            panic!("Expected curve");
        };
        assert_eq!((curve.p1().x(), curve.p1().y()), (10.0, 20.0));
        doc.replace_page_contents(index, b"10 20 m 30 40 l n S".to_vec())
            .unwrap();
        assert!(
            doc.get_page(index)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .is_empty()
        );
    }

    #[test]
    fn shorthand_y_curve_uses_end_point_as_second_control() {
        use crate::{page::page_object::PageObject, path::pdf_path::Segment};
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"10 20 m 30 40 50 60 y S".to_vec())
            .unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("Expected path");
        };
        let Segment::CurveTo(curve) = &path.pdf_path().subpaths()[0].segments()[1] else {
            panic!("Expected curve");
        };
        assert_eq!((curve.p1().x(), curve.p1().y()), (30.0, 40.0));
        assert_eq!((curve.p2().x(), curve.p2().y()), (50.0, 60.0));
        assert_eq!((curve.p3().x(), curve.p3().y()), (50.0, 60.0));
    }
}

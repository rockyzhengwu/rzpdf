use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
mod feature_versions;
mod incremental;
mod protection;
pub(crate) use incremental::{write_incremental, write_signature_incremental};

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

pub(crate) fn write_document_with_budget<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    options: crate::document::save_options::SaveOptions,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut output = SerializationOutput {
        bytes: crate::budget::BudgetedBytes::copy_from_slice(budget, &[])?,
        limits: document.ctx().limits(),
        capacity_limit: options.max_output_bytes,
    };
    write_document_output(document, options, &mut output)?;
    Ok(output.bytes)
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
    if document.ctx().source_security().is_some() {
        let budget = document.ctx().limits().operation_budget();
        let bytes = write_protected_document_with_budget(
            document,
            options,
            crate::security::SaveSecurity::Preserve,
            &budget,
        )?;
        return Ok(bytes.into_reserved_parts().0);
    }
    let mut output = Vec::new();
    write_document_output(document, options, &mut output)?;
    Ok(output)
}

fn write_document_output<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    options: crate::document::save_options::SaveOptions,
    output: &mut impl OutputBuffer,
) -> PdfResult<()> {
    write_document_output_with_security(
        document,
        options,
        crate::security::SaveSecurity::Preserve,
        output,
    )
}

fn write_protected_document_with_budget<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    options: crate::document::save_options::SaveOptions,
    policy: crate::security::SaveSecurity<'_>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut output = SerializationOutput {
        bytes: crate::budget::BudgetedBytes::copy_from_slice(budget, &[])?,
        limits: document.ctx().limits(),
        capacity_limit: options.max_output_bytes,
    };
    write_document_output_with_security(document, options, policy, &mut output)?;
    Ok(output.bytes)
}

fn write_document_output_with_security<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    mut options: crate::document::save_options::SaveOptions,
    policy: crate::security::SaveSecurity<'_>,
    mut output: &mut impl OutputBuffer,
) -> PdfResult<()> {
    // Validate the latest Catalog override as well as the source Header.
    let budget = output.budget();
    let crypto_budget = budget
        .clone()
        .unwrap_or_else(|| document.ctx().limits().operation_budget());
    let source_security = document.ctx().source_security();
    document
        .ctx()
        .require_unsigned_source_with_budget(&crypto_budget)?;
    if source_security.is_none()
        && document
            .ctx()
            .trailer()
            .get("Encrypt")
            .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
    {
        return Err(PdfError::UnsupportedSecurity(
            "unauthenticated encryption dictionary".into(),
        ));
    }
    if !matches!(policy, crate::security::SaveSecurity::Preserve)
        && source_security.is_some_and(|handler| {
            handler.report().authentication != crate::security::Authentication::Owner
        })
    {
        return Err(PdfError::PermissionDenied(
            "changing document protection requires owner authentication".into(),
        ));
    }
    let mut _new_security_lease = None;
    let new_security = match policy {
        crate::security::SaveSecurity::Set(settings) => {
            if options
                .version
                .is_some_and(|version| version < crate::document::save_options::PdfVersion::V2_0)
            {
                return Err(PdfError::UnsupportedSecurity(
                    "new AES-256 output requires PDF 2.0".into(),
                ));
            }
            options.version = Some(crate::document::save_options::PdfVersion::V2_0);
            let (handler, lease) = crate::security::SecurityHandler::generate(
                settings,
                document.ctx().limits(),
                &crypto_budget,
            )?;
            _new_security_lease = Some(lease);
            Some(handler)
        }
        _ => None,
    };
    let security = match policy {
        crate::security::SaveSecurity::Preserve => source_security,
        crate::security::SaveSecurity::Remove => None,
        crate::security::SaveSecurity::Set(_) => new_security.as_ref(),
    };
    let effective = match budget.as_ref() {
        Some(budget) => document.effective_version_with_budget(budget)?,
        None => document.effective_version()?,
    };
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
    if security.is_some() {
        document.validate_security_version(
            effective.max(options.version.unwrap_or(effective)),
            &crypto_budget,
        )?;
    }
    validate_serialization_nodes_with_budget(
        document
            .ctx()
            .trailer()
            .into_iter()
            .map(|(_, value)| (value, 2)),
        1,
        document.ctx().limits(),
        budget.as_ref(),
    )?;
    if let Some(budget) = budget.as_ref() {
        document.ctx().materialize_all_objects_with_budget(budget)?;
    } else {
        document.ctx().materialize_all_objects()?;
    }

    append_formatted_output(
        &mut output,
        format_args!("%PDF-{version}\n"),
        options.max_output_bytes,
    )?;
    append_output(
        &mut output,
        b"%\xE2\xE3\xCF\xD3\n",
        options.max_output_bytes,
    )?;

    let (_ids_lease, object_ids) = if let Some(budget) = budget.as_ref() {
        let (ids, lease) = document.ctx().object_ids_with_budget(true, budget)?;
        (Some(lease), ids)
    } else {
        (None, document.ctx().serializable_object_ids())
    };
    let max_obj_num = object_ids.iter().map(|id| id.obj_num()).max().unwrap_or(0);
    let encryption_id = if security.is_some() {
        Some(ObjectId::new(
            max_obj_num
                .checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("Encrypt object number overflow".into()))?,
            0,
        ))
    } else {
        None
    };
    let size = encryption_id
        .map(|id| id.obj_num())
        .unwrap_or(max_obj_num)
        .checked_add(1)
        .ok_or_else(|| PdfError::ResourceLimit("xref output Size overflow".into()))?;
    let (_xref_lease, mut xref_entries) = {
        let (entries, lease) = allocate_xref_entries_inner(
            size,
            document.ctx().limits().max_objects,
            budget.as_ref(),
        )?;
        (lease, entries)
    };
    if !xref_entries.is_empty() {
        xref_entries[0] = (0, 65535, b'f');
    }

    for object_id in object_ids {
        let mut _object_lease = None;
        let object = if let Some(budget) = budget.as_ref() {
            document
                .ctx()
                .object_for_write_with_budget(object_id, budget)?
                .map(|(object, lease)| {
                    _object_lease = Some(lease);
                    object
                })
        } else {
            document.ctx().object_for_write(object_id)?
        };
        if let Some(obj) = object {
            validate_serialization_nodes_with_budget(
                std::iter::once((&obj, 1)),
                0,
                document.ctx().limits(),
                budget.as_ref(),
            )?;
            let mut _prepared_lease = None;
            let obj = if let Some(budget) = budget.as_ref() {
                let (object, lease) = prepare_document_output_object_with_budget(
                    document,
                    obj,
                    options.compression,
                    budget,
                )?;
                _prepared_lease = Some(lease);
                object
            } else {
                prepare_document_output_object(document, obj, options.compression)?
            };
            validate_serialization_nodes_with_budget(
                std::iter::once((&obj, 1)),
                0,
                document.ctx().limits(),
                budget.as_ref(),
            )?;
            feature_versions::validate_feature_versions_with_budget(
                document,
                &obj,
                effective.max(options.version.unwrap_or(effective)),
                budget.as_ref(),
            )?;
            let mut _encoded_lease = None;
            let obj = if let Some(handler) = security {
                let (object, lease) =
                    handler.transform(document.ctx(), &obj, true, &crypto_budget)?;
                _encoded_lease = Some(lease);
                object
            } else if source_security.is_some() {
                let (object, lease) =
                    crate::security::transform::plain_output(document.ctx(), &obj, &crypto_budget)?;
                _encoded_lease = Some(lease);
                object
            } else {
                obj
            };
            xref_entries[object_id.obj_num() as usize] =
                (output.len() as u64, object_id.gen_num(), b'n');
            append_formatted_output(
                &mut output,
                format_args!("{} {} obj\n", object_id.obj_num(), object_id.gen_num()),
                options.max_output_bytes,
            )?;
            write_object_limited(&mut output, &obj, options.max_output_bytes)?;
            append_output(&mut output, b"\nendobj\n", options.max_output_bytes)?;
        }
    }

    if let (Some(id), Some(handler)) = (encryption_id, security) {
        validate_serialization_nodes_with_budget(
            handler.dictionary.into_iter().map(|(_, value)| (value, 2)),
            1,
            document.ctx().limits(),
            Some(&crypto_budget),
        )?;
        xref_entries[id.obj_num() as usize] = (output.len() as u64, 0, b'n');
        append_formatted_output(
            &mut output,
            format_args!("{} 0 obj\n", id.obj_num()),
            options.max_output_bytes,
        )?;
        write_dict_limited(&mut output, &handler.dictionary, options.max_output_bytes)?;
        append_output(&mut output, b"\nendobj\n", options.max_output_bytes)?;
    }

    let xref_offset = output.len() as u64;
    write_xref_table(&mut output, &xref_entries, options.max_output_bytes)?;
    let mut _protected_trailer_lease = None;
    let protected_trailer = if security.is_some()
        || source_security.is_some()
        || matches!(policy, crate::security::SaveSecurity::Remove)
    {
        let (dict, lease) = protection::trailer(
            &document.ctx().trailer(),
            encryption_id,
            document.ctx().limits(),
            &crypto_budget,
        )?;
        _protected_trailer_lease = Some(lease);
        Some(dict)
    } else {
        None
    };
    write_trailer_limited(
        &mut output,
        protected_trailer
            .as_ref()
            .unwrap_or(&document.ctx().trailer()),
        size,
        document.ctx().limits(),
        options.max_output_bytes,
    )?;
    append_output(&mut output, b"startxref\n", options.max_output_bytes)?;
    append_formatted_output(
        &mut output,
        format_args!("{xref_offset}\n"),
        options.max_output_bytes,
    )?;
    append_output(&mut output, b"%%EOF\n", options.max_output_bytes)?;
    Ok(())
}

trait OutputBuffer {
    fn limits(&self) -> crate::parser::options::Limits {
        Default::default()
    }
    fn len(&self) -> usize;
    fn append(&mut self, bytes: &[u8]) -> PdfResult<()>;
    fn budget(&self) -> Option<crate::budget::OperationBudget> {
        None
    }
}

impl<T: OutputBuffer + ?Sized> OutputBuffer for &mut T {
    fn limits(&self) -> crate::parser::options::Limits {
        (**self).limits()
    }
    fn len(&self) -> usize {
        (**self).len()
    }
    fn append(&mut self, bytes: &[u8]) -> PdfResult<()> {
        (**self).append(bytes)
    }
    fn budget(&self) -> Option<crate::budget::OperationBudget> {
        (**self).budget()
    }
}

struct SerializationOutput {
    bytes: crate::budget::BudgetedBytes,
    limits: crate::parser::options::Limits,
    capacity_limit: usize,
}

impl OutputBuffer for SerializationOutput {
    fn limits(&self) -> crate::parser::options::Limits {
        self.limits
    }
    fn len(&self) -> usize {
        self.bytes.as_slice().len()
    }
    fn append(&mut self, bytes: &[u8]) -> PdfResult<()> {
        let needed = self
            .bytes
            .as_slice()
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| PdfError::ResourceLimit("serialized output length".into()))?;
        if needed > self.bytes.capacity() {
            let capacity = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(needed)
                .min(self.capacity_limit);
            self.bytes.try_reserve_capacity(capacity)?;
        }
        self.bytes.try_extend_from_slice(bytes)
    }
    fn budget(&self) -> Option<crate::budget::OperationBudget> {
        Some(self.bytes.operation_budget())
    }
}

impl OutputBuffer for Vec<u8> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn append(&mut self, bytes: &[u8]) -> PdfResult<()> {
        self.try_reserve_exact(bytes.len())
            .map_err(|_| PdfError::ResourceLimit("PDF output allocation".into()))?;
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl OutputBuffer for crate::budget::BudgetedBytes {
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    fn append(&mut self, bytes: &[u8]) -> PdfResult<()> {
        self.try_extend_from_slice(bytes)
    }
    fn budget(&self) -> Option<crate::budget::OperationBudget> {
        Some(self.operation_budget())
    }
}

fn append_output(output: &mut impl OutputBuffer, bytes: &[u8], limit: usize) -> PdfResult<()> {
    crate::filter::check_output_size(output.len(), bytes.len(), limit, "PDF output bytes")?;
    output.append(bytes)
}

pub(crate) fn serialize_object_with_budget(
    object: &PdfObject,
    limits: crate::parser::options::Limits,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    validate_serialization_nodes_with_budget(
        std::iter::once((object, 1)),
        0,
        limits,
        Some(budget),
    )?;
    let mut output = SerializationOutput {
        bytes: crate::budget::BudgetedBytes::copy_from_slice(budget, &[])?,
        limits,
        capacity_limit: limits.max_decoded_bytes,
    };
    write_object_limited(&mut output, object, limits.max_decoded_bytes)?;
    Ok(output.bytes)
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

fn compress_with_budget(
    input: &[u8],
    budget: &crate::budget::OperationBudget,
    limits: crate::parser::options::Limits,
) -> PdfResult<crate::budget::BudgetedBytes> {
    // Own the staging allocation so reservation precedes allocation and actual
    // capacity can be reconciled. Backend-private state remains excluded.
    let mut _workspace_lease = budget.reserve(32 * 1024)?;
    let mut workspace = Vec::new();
    workspace
        .try_reserve_exact(32 * 1024)
        .map_err(|_| PdfError::ResourceLimit("compression workspace allocation".into()))?;
    _workspace_lease.resize(workspace.capacity())?;
    workspace.resize(32 * 1024, 0);
    let mut encoder = flate2::Compress::new(flate2::Compression::default(), true);
    let mut output = SerializationOutput {
        bytes: crate::budget::BudgetedBytes::copy_from_slice(budget, &[])?,
        limits,
        capacity_limit: limits.max_raw_stream_bytes,
    };
    let mut position = 0;
    loop {
        let before_in = encoder.total_in();
        let before_out = encoder.total_out();
        let flush = if position == input.len() {
            flate2::FlushCompress::Finish
        } else {
            flate2::FlushCompress::None
        };
        let status = encoder
            .compress(&input[position..], &mut workspace, flush)
            .map_err(|_| PdfError::IOError("compress output stream failed".into()))?;
        let consumed = usize::try_from(encoder.total_in() - before_in)
            .map_err(|_| PdfError::ResourceLimit("compression input progress".into()))?;
        let produced = usize::try_from(encoder.total_out() - before_out)
            .map_err(|_| PdfError::ResourceLimit("compression output progress".into()))?;
        position = position
            .checked_add(consumed)
            .filter(|position| *position <= input.len())
            .ok_or_else(|| PdfError::IOError("invalid compressor progress".into()))?;
        append_output(
            &mut output,
            &workspace[..produced],
            limits.max_raw_stream_bytes,
        )?;
        if status == flate2::Status::StreamEnd {
            if position != input.len() {
                return Err(PdfError::IOError("incomplete compression input".into()));
            }
            return Ok(output.bytes);
        }
        if consumed == 0 && produced == 0 {
            return Err(PdfError::IOError("compressor made no progress".into()));
        }
    }
}

fn prepare_document_output_object_with_budget<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    object: PdfObject,
    compression: crate::document::save_options::StreamCompression,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
    use crate::{
        document::save_options::StreamCompression,
        objects::pdf_stream::resolve_setting_reference_with_budget,
    };
    let mut lease = budget.reserve(0)?;
    if !matches!(compression, StreamCompression::FlateUnencoded) {
        return Ok((object, lease));
    }
    let PdfObject::PdfStream(stream) = object else {
        return Ok((object, lease));
    };
    if ["F", "FFilter", "FDecodeParms"]
        .iter()
        .any(|key| stream.dict().contains_key(key))
    {
        return Ok((PdfObject::PdfStream(stream), lease));
    }
    if let Some(filter) = stream.dict().get("Filter") {
        let (_filter_lease, filter) = {
            let (filter, lease) =
                resolve_setting_reference_with_budget(filter, document.ctx(), budget)?;
            (lease, filter)
        };
        if !matches!(filter, PdfObject::PdfNull) {
            return Ok((PdfObject::PdfStream(stream), lease));
        }
    }
    if let Some(params) = stream.dict().get("DecodeParms") {
        let (_params_lease, params) = {
            let (params, lease) =
                resolve_setting_reference_with_budget(params, document.ctx(), budget)?;
            (lease, params)
        };
        if !matches!(params, PdfObject::PdfNull) {
            return Err(PdfError::UnsupportedFeature(
                "cannot compress unfiltered stream with DecodeParms".into(),
            ));
        }
    }
    let limits = document.ctx().limits();
    crate::filter::check_output_size(
        0,
        stream.raw_data().len(),
        limits.max_raw_stream_bytes.min(limits.max_decoded_bytes),
        "unencoded compression input bytes",
    )?;
    let bytes = compress_with_budget(stream.raw_data(), budget, limits)?;
    let (_name_lease, name) = {
        let (name, lease) = PdfName::new_from_buffer_with_budget(b"/FlateDecode", budget)?;
        (lease, PdfObject::PdfName(name))
    };
    let (mut dict_lease, mut dict) = {
        let (dict, lease) = stream
            .dict()
            .with_replaced_entry_with_budget("Filter", &name, budget, limits)?;
        (lease, dict)
    };
    dict.remove("DecodeParms");
    lease.absorb(&mut dict_lease)?;
    let (mut data_lease, data) = {
        let (data, lease) = bytes.into_reserved_parts();
        (lease, data)
    };
    lease.absorb(&mut data_lease)?;
    Ok((PdfObject::PdfStream(PdfStream::new(dict, data)), lease))
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
    nodes: usize,
    limits: crate::parser::options::Limits,
) -> PdfResult<()> {
    validate_serialization_nodes_with_budget(roots, nodes, limits, None)
}

fn reserve_validation_slots<T>(
    pending: &mut Vec<T>,
    additional: usize,
    lease: &mut Option<crate::budget::MemoryLease>,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<()> {
    let needed = pending
        .len()
        .checked_add(additional)
        .ok_or_else(|| PdfError::ResourceLimit("serialization validation slots".into()))?;
    if needed <= pending.capacity() {
        return Ok(());
    }
    if let Some(budget) = budget {
        let slot_size = std::mem::size_of::<T>();
        let capacity = needed.max(pending.capacity().saturating_mul(2));
        let mut next_lease =
            budget.reserve(capacity.checked_mul(slot_size).ok_or_else(|| {
                PdfError::ResourceLimit("serialization validation bytes".into())
            })?)?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("serialization validation allocation".into()))?;
        next_lease.resize(next.capacity().checked_mul(slot_size).ok_or_else(|| {
            PdfError::ResourceLimit("serialization validation capacity".into())
        })?)?;
        next.append(pending);
        *pending = next;
        *lease = Some(next_lease);
        Ok(())
    } else {
        pending
            .try_reserve(additional)
            .map_err(|_| PdfError::ResourceLimit("serialization validation allocation".into()))
    }
}

fn validate_serialization_nodes_with_budget<'a>(
    roots: impl Iterator<Item = (&'a PdfObject, usize)>,
    mut nodes: usize,
    limits: crate::parser::options::Limits,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<()> {
    crate::filter::check_output_size(0, nodes, limits.max_objects, "serialized object nodes")?;
    if nodes > 0 && limits.max_depth == 0 {
        return Err(PdfError::ResourceLimit("serialized object depth".into()));
    }
    let mut _pending_lease = None;
    let mut pending = Vec::new();
    for root in roots {
        let used = nodes
            .checked_add(pending.len())
            .ok_or_else(|| PdfError::ResourceLimit("serialized nodes overflow".into()))?;
        crate::filter::check_output_size(used, 1, limits.max_objects, "serialized object nodes")?;
        reserve_validation_slots(&mut pending, 1, &mut _pending_lease, budget)?;
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
                reserve_validation_slots(&mut pending, array.len(), &mut _pending_lease, budget)?;
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
                    reserve_validation_slots(&mut pending, 1, &mut _pending_lease, budget)?;
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
                    reserve_validation_slots(&mut pending, 1, &mut _pending_lease, budget)?;
                    pending.push((child, next_depth));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn allocate_xref_entries(size: u32, limit: usize) -> PdfResult<Vec<(u64, u32, u8)>> {
    allocate_xref_entries_inner(size, limit, None).map(|(entries, _)| entries)
}

fn allocate_xref_entries_inner(
    size: u32,
    limit: usize,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(Vec<(u64, u32, u8)>, Option<crate::budget::MemoryLease>)> {
    let size = usize::try_from(size)
        .map_err(|_| PdfError::ResourceLimit("xref output size overflow".into()))?;
    crate::filter::check_output_size(0, size, limit, "xref output entries")?;
    let mut lease = budget
        .map(|budget| {
            budget.reserve(
                size.checked_mul(std::mem::size_of::<(u64, u32, u8)>())
                    .ok_or_else(|| PdfError::ResourceLimit("xref output table size".into()))?,
            )
        })
        .transpose()?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(size)
        .map_err(|_| PdfError::ResourceLimit("xref output allocation".into()))?;
    if let Some(lease) = lease.as_mut() {
        lease.resize(
            entries
                .capacity()
                .checked_mul(std::mem::size_of::<(u64, u32, u8)>())
                .ok_or_else(|| PdfError::ResourceLimit("xref output table capacity".into()))?,
        )?;
    }
    entries.resize(size, (0, 0, b'f'));
    Ok((entries, lease))
}

fn write_indirect_object(
    output: &mut Vec<u8>,
    object_id: ObjectId,
    obj: &PdfObject,
) -> PdfResult<()> {
    append_formatted_output(
        output,
        format_args!("{} {} obj\n", object_id.obj_num(), object_id.gen_num()),
        usize::MAX,
    )?;
    write_object(output, obj)?;
    output.extend_from_slice(b"\nendobj\n");
    Ok(())
}

struct StackFormat {
    bytes: [u8; 64],
    len: usize,
}
impl std::fmt::Write for StackFormat {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self
            .len
            .checked_add(value.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or(std::fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}
fn format_stack(arguments: std::fmt::Arguments<'_>) -> PdfResult<StackFormat> {
    let mut token = StackFormat {
        bytes: [0; 64],
        len: 0,
    };
    std::fmt::Write::write_fmt(&mut token, arguments)
        .map_err(|_| PdfError::ResourceLimit("writer stack token capacity".into()))?;
    Ok(token)
}
fn append_formatted_output(
    output: &mut impl OutputBuffer,
    arguments: std::fmt::Arguments<'_>,
    limit: usize,
) -> PdfResult<()> {
    let token = format_stack(arguments)?;
    append_output(output, &token.bytes[..token.len], limit)
}

fn write_xref_table(
    output: &mut impl OutputBuffer,
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
    append_formatted_output(output, format_args!("0 {}\n", entries.len()), limit)?;
    for (offset, gen_num, state) in entries {
        append_formatted_output(
            output,
            format_args!("{offset:010} {gen_num:05} {} \n", *state as char),
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
    output: &mut impl OutputBuffer,
    original: &PdfDict,
    size: u32,
    limits: crate::parser::options::Limits,
    output_limit: usize,
) -> PdfResult<()> {
    // Keep leases outside their owned values so unwinding frees data first.
    let mut _size_lease = None;
    let mut digits = [0u8; 10];
    let mut start = digits.len();
    let mut remaining = size;
    loop {
        start -= 1;
        digits[start] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let size = if let Some(budget) = output.budget() {
        let (lexeme, lease) =
            crate::budget::copy_string(std::str::from_utf8(&digits[start..]).unwrap(), &budget)?;
        _size_lease = Some(lease);
        crate::objects::pdf_number::PdfNumber::from_owned_lexeme(lexeme)?
    } else {
        crate::objects::pdf_number::PdfNumber::new_from_lexeme(&digits[start..])?
    };
    let size = PdfObject::PdfNumber(size);
    let (_trailer_lease, trailer) = if let Some(budget) = output.budget() {
        let mut builder =
            crate::objects::pdf_dict::BudgetedDictBuilder::new(&budget, limits.max_objects)?;
        for (key, value) in original {
            if !retained_trailer_key(key) || key == "Size" {
                continue;
            }
            if key.len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("trailer key bytes".into()));
            }
            // Declare the key lease before its data, including value-copy errors.
            let (key_lease, key) = {
                let (key, lease) = crate::budget::copy_string(key, &budget)?;
                (lease, key)
            };
            let (value, value_lease) =
                crate::objects::clone_object_with_budget(value, &budget, limits, 1)?;
            builder.insert(key, value, key_lease, value_lease)?;
        }
        let (key_lease, key) = {
            let (key, lease) = crate::budget::copy_string("Size", &budget)?;
            (lease, key)
        };
        let (value, value_lease) =
            crate::objects::clone_object_with_budget(&size, &budget, limits, 1)?;
        builder.insert(key, value, key_lease, value_lease)?;
        let (trailer, lease) = builder.finish()?;
        (Some(lease), trailer)
    } else {
        let mut trailer = PdfDict::default();
        for (key, value) in original {
            if retained_trailer_key(key) {
                trailer.try_insert(key.clone(), value.clone())?;
            }
        }
        trailer.try_insert("Size".to_string(), size)?;
        (None, trailer)
    };
    validate_serialization_nodes_with_budget(
        trailer.into_iter().map(|(_, value)| (value, 2)),
        1,
        limits,
        output.budget().as_ref(),
    )?;
    append_output(output, b"trailer\n", output_limit)?;
    write_dict_limited(output, &trailer, output_limit)?;
    append_output(output, b"\n", output_limit)?;
    Ok(())
}

fn retained_trailer_key(key: &str) -> bool {
    !matches!(
        key,
        "Prev" | "XRefStm" | "Type" | "W" | "Index" | "Filter" | "DecodeParms" | "Length"
    )
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
        PdfObject::PdfReference(reference) => append_formatted_output(
            output,
            format_args!("{} {} R", reference.objnum(), reference.gennum()),
            usize::MAX,
        )?,
    }
    Ok(())
}

fn write_object_limited(
    output: &mut impl OutputBuffer,
    object: &PdfObject,
    limit: usize,
) -> PdfResult<()> {
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
        PdfObject::PdfNumber(number) => {
            append_output(output, validated_number_lexeme(number)?.as_bytes(), limit)
        }
        PdfObject::PdfStream(stream) => {
            let mut _length_lease = None;
            let length = if let Some(budget) = output.budget() {
                let mut buffer = [0u8; 20];
                let mut start = buffer.len();
                let mut value = stream.raw_data().len();
                loop {
                    start -= 1;
                    buffer[start] = b'0' + (value % 10) as u8;
                    value /= 10;
                    if value == 0 {
                        break;
                    }
                }
                let (lexeme, lease) = crate::budget::copy_string(
                    std::str::from_utf8(&buffer[start..]).unwrap(),
                    &budget,
                )?;
                _length_lease = Some(lease);
                crate::objects::pdf_number::PdfNumber::from_owned_lexeme(lexeme)?
            } else {
                crate::objects::pdf_number::PdfNumber::new_from_lexeme(
                    stream.raw_data().len().to_string().as_bytes(),
                )?
            };
            let length = PdfObject::PdfNumber(length);
            let (_dict_lease, dict) = if let Some(budget) = output.budget() {
                let (dict, lease) = stream.dict().with_replaced_entry_with_budget(
                    "Length",
                    &length,
                    &budget,
                    output.limits(),
                )?;
                (Some(lease), dict)
            } else {
                let mut dict = stream.dict().clone();
                dict.try_insert("Length".into(), length)?;
                (None, dict)
            };
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
                    format_stack(format_args!(
                        "{} {} R",
                        reference.objnum(),
                        reference.gennum()
                    ))?
                    .len
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
            let mut token_lease = output
                .budget()
                .map(|budget| budget.reserve(token_length))
                .transpose()?;
            let mut token = Vec::new();
            if let Some(lease) = token_lease.as_mut() {
                token
                    .try_reserve_exact(token_length)
                    .map_err(|_| PdfError::ResourceLimit("PDF token allocation".into()))?;
                lease.resize(token.capacity())?;
            }
            write_object(&mut token, object)?;
            append_output(output, &token, limit)
        }
    }
}

fn write_dict_limited(
    output: &mut impl OutputBuffer,
    dict: &PdfDict,
    limit: usize,
) -> PdfResult<()> {
    let budget = output.budget();
    let mut _entries_lease = budget
        .as_ref()
        .map(|budget| {
            budget.reserve(
                dict.into_iter()
                    .len()
                    .checked_mul(std::mem::size_of::<(&String, &PdfObject)>())
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("serialization dictionary table size".into())
                    })?,
            )
        })
        .transpose()?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(dict.into_iter().len())
        .map_err(|_| PdfError::ResourceLimit("serialization dictionary table allocation".into()))?;
    if let Some(lease) = _entries_lease.as_mut() {
        lease.resize(
            entries
                .capacity()
                .checked_mul(std::mem::size_of::<(&String, &PdfObject)>())
                .ok_or_else(|| {
                    PdfError::ResourceLimit("serialization dictionary table capacity".into())
                })?,
        )?;
    }
    entries.extend(dict);
    entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    append_output(output, b"<<", limit)?;
    for (key, value) in entries {
        append_output(output, b"\n", limit)?;
        let length = serialized_name_bytes_length(key.as_bytes())?;
        crate::filter::check_output_size(output.len(), length, limit, "PDF output token bytes")?;
        let mut _name_lease = output
            .budget()
            .map(|budget| budget.reserve(length))
            .transpose()?;
        let mut name = Vec::new();
        if let Some(lease) = _name_lease.as_mut() {
            name.try_reserve_exact(length)
                .map_err(|_| PdfError::ResourceLimit("PDF name allocation".into()))?;
            lease.resize(name.capacity())?;
        }
        write_name_bytes(&mut name, key.as_bytes())?;
        append_output(output, &name, limit)?;
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
    serialized_name_bytes_length(name.bytes())
}

pub(crate) fn serialized_name_bytes_length(bytes: &[u8]) -> PdfResult<usize> {
    if bytes.contains(&0) {
        return Err(PdfError::ObjectError("name contains zero byte".into()));
    }
    bytes.iter().try_fold(1usize, |length, byte| {
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
    write_name_bytes(output, name.bytes())
}

pub(crate) fn write_name_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> PdfResult<()> {
    let length = serialized_name_bytes_length(bytes)?;
    let escaped = |byte: u8| !(33..=126).contains(&byte) || b"()<>[]{}/%#".contains(&byte);
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("serialized name allocation".into()))?;
    output.extend_from_slice(b"/");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &byte in bytes {
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
    Ok(validated_number_lexeme(number)?.to_owned())
}

fn validated_number_lexeme(number: &crate::objects::pdf_number::PdfNumber) -> PdfResult<&str> {
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
    Ok(lexeme)
}

impl<R: std::io::Read + std::io::Seek> PdfDocument<R> {
    pub fn save_to_bytes_with_security(
        &self,
        options: crate::document::save_options::SaveOptions,
        policy: crate::security::SaveSecurity<'_>,
    ) -> PdfResult<Vec<u8>> {
        let budget = self.ctx().limits().operation_budget();
        Ok(self
            .save_with_security_and_budget(options, policy, &budget)?
            .into_reserved_parts()
            .0)
    }

    pub fn save_with_security_and_budget(
        &self,
        options: crate::document::save_options::SaveOptions,
        policy: crate::security::SaveSecurity<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        write_protected_document_with_budget(self, options, policy, budget)
    }

    pub fn save_to_writer_with_security<W: Write>(
        &self,
        output: &mut W,
        options: crate::document::save_options::SaveOptions,
        policy: crate::security::SaveSecurity<'_>,
    ) -> PdfResult<()> {
        output.write_all(&self.save_to_bytes_with_security(options, policy)?)?;
        Ok(())
    }

    pub fn save_to_file_with_security<P: AsRef<Path>>(
        &self,
        path: P,
        options: crate::document::save_options::SaveOptions,
        policy: crate::security::SaveSecurity<'_>,
    ) -> PdfResult<()> {
        save_bytes_atomically(
            path.as_ref(),
            &self.save_to_bytes_with_security(options, policy)?,
        )
    }

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

pub(crate) fn save_bytes_atomically(path: &Path, bytes: &[u8]) -> PdfResult<()> {
    save_file_atomically(path, |file| file.write_all(bytes))
}

fn save_file_atomically(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> PdfResult<()> {
    save_file_atomically_with_actions(path, write, std::fs::File::sync_all, |source, target| {
        std::fs::rename(source, target)
    })
}

fn save_file_atomically_with_actions(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
    sync: impl FnOnce(&std::fs::File) -> std::io::Result<()>,
    replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
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
    // Locals unwind in reverse declaration order: close the file before the
    // path guard attempts removal, not only on explicit Result error paths.
    let (mut temporary, mut file) = {
        let (file, temporary) = allocated
            .ok_or_else(|| PdfError::IOError("temporary output name attempts exhausted".into()))?;
        (temporary, file)
    };
    let prepared = (|| {
        write(&mut file)
            .map_err(|error| PdfError::IOError(format!("write temporary PDF failed: {error}")))?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions).map_err(|error| {
                PdfError::IOError(format!("set output permissions failed: {error}"))
            })?;
        }
        sync(&file)
            .map_err(|error| PdfError::IOError(format!("sync temporary PDF failed: {error}")))
    })();
    // Close before cleanup on success and error paths, including Windows.
    drop(file);
    prepared?;
    replace(&temporary.0, path)
        .map_err(|error| PdfError::IOError(format!("replace output file failed: {error}")))?;
    temporary.1 = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn compression_workspace_budget_handles_empty_and_multibuffer_streams() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        use std::io::Write;
        let inputs = [
            vec![],
            (0..=255).collect(),
            (0u32..32768).flat_map(u32::to_le_bytes).collect(),
        ];
        for input in inputs {
            let mut ordinary =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            ordinary.write_all(&input).unwrap();
            let expected = ordinary.finish().unwrap();
            let pool = OperationBudget::new(1024 * 1024);
            let output = super::compress_with_budget(&input, &pool, Limits::default()).unwrap();
            assert_eq!(output.as_slice(), expected);
            assert_eq!(pool.live_bytes(), output.capacity());
            assert!(pool.peak_bytes() >= 32 * 1024 + output.capacity());
            let peak = pool.peak_bytes();
            let mut decoded = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::ZlibDecoder::new(output.as_slice()),
                &mut decoded,
            )
            .unwrap();
            assert_eq!(decoded, input);
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
            let insufficient = OperationBudget::new(32 * 1024 - 1);
            assert!(super::compress_with_budget(&input, &insufficient, Limits::default()).is_err());
            assert_eq!(insufficient.live_bytes(), 0);
            let insufficient = OperationBudget::new(peak - 1);
            assert!(super::compress_with_budget(&input, &insufficient, Limits::default()).is_err());
            assert_eq!(insufficient.live_bytes(), 0);
            assert!(
                super::compress_with_budget(
                    &input,
                    &pool,
                    Limits {
                        max_raw_stream_bytes: expected.len() - 1,
                        ..Default::default()
                    }
                )
                .is_err()
            );
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn stack_writer_tokens_preserve_bounds_and_xref_bytes() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let token = super::format_stack(format_args!("{} {} R", u32::MAX, u32::MAX)).unwrap();
        assert_eq!(&token.bytes[..token.len], b"4294967295 4294967295 R");
        let token = super::format_stack(format_args!("{}\n", u64::MAX)).unwrap();
        assert_eq!(&token.bytes[..token.len], b"18446744073709551615\n");
        let long = [b'x'; 65];
        let mut unchanged = b"prefix".to_vec();
        assert!(
            super::append_formatted_output(
                &mut unchanged,
                format_args!("{}", std::str::from_utf8(&long).unwrap()),
                usize::MAX
            )
            .is_err()
        );
        assert_eq!(unchanged, b"prefix");
        let entries = [(9_999_999_999, 65535, b'f'), (0, 0, b'n')];
        let expected = b"xref\n0 2\n9999999999 65535 f \n0000000000 00000 n \n";
        let mut plain = Vec::new();
        super::write_xref_table(&mut plain, &entries, expected.len()).unwrap();
        assert_eq!(plain, expected);
        let pool = OperationBudget::new(1024);
        let mut output = super::SerializationOutput {
            bytes: crate::budget::BudgetedBytes::copy_from_slice(&pool, &[]).unwrap(),
            limits: Limits::default(),
            capacity_limit: expected.len(),
        };
        super::write_xref_table(&mut output, &entries, expected.len()).unwrap();
        assert_eq!(output.bytes.as_slice(), expected);
        assert_eq!(pool.live_bytes(), output.bytes.capacity());
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        for invalid in [(10_000_000_000, 0, b'n'), (0, 65536, b'n'), (0, 0, b'?')] {
            let mut output = Vec::new();
            assert!(super::write_xref_table(&mut output, &[invalid], usize::MAX).is_err());
            assert!(output.is_empty());
        }
    }

    #[test]
    fn compressed_output_budget_retains_payload_and_rebuilt_dictionary() {
        use crate::{
            budget::OperationBudget,
            document::save_options::StreamCompression,
            objects::{pdf_reference::PdfReference, pdf_stream::PdfStream},
        };
        let doc = PdfDocument::new().unwrap();
        let null = doc.insert_object(PdfObject::PdfNull);
        let mut dict = PdfDict::default();
        for key in ["Filter", "DecodeParms"] {
            dict.insert(
                key.into(),
                PdfObject::PdfReference(PdfReference::new(null.obj_num(), null.gen_num())),
            );
        }
        dict.insert(
            "Custom".into(),
            PdfObject::PdfString(PdfString::new(vec![b'x'; 4096], false)),
        );
        let source = PdfObject::PdfStream(PdfStream::new(
            dict.clone(),
            (0..16384).map(|i| (i % 251) as u8).collect(),
        ));
        let revision = doc.revision();
        let ordinary = super::prepare_document_output_object(
            &doc,
            source.clone(),
            StreamCompression::FlateUnencoded,
        )
        .unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        let (lease, object) = {
            let (object, lease) = super::prepare_document_output_object_with_budget(
                &doc,
                source.clone(),
                StreamCompression::FlateUnencoded,
                &pool,
            )
            .unwrap();
            (lease, object)
        };
        assert_eq!(ordinary, object);
        assert!(pool.live_bytes() > 4096);
        let peak = pool.peak_bytes();
        drop(object);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        let small = OperationBudget::new(peak - 1);
        assert!(matches!(
            super::prepare_document_output_object_with_budget(
                &doc,
                source.clone(),
                StreamCompression::FlateUnencoded,
                &small
            ),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (_lease, _object) = {
                let (object, lease) = super::prepare_document_output_object_with_budget(
                    &doc,
                    source.clone(),
                    StreamCompression::FlateUnencoded,
                    &pool,
                )
                .unwrap();
                (lease, object)
            };
            panic!("compressed owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        dict.insert("F".into(), PdfObject::PdfNull);
        let external = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
        let zero = OperationBudget::new(0);
        let (preserved, lease) = super::prepare_document_output_object_with_budget(
            &doc,
            external.clone(),
            StreamCompression::FlateUnencoded,
            &zero,
        )
        .unwrap();
        assert_eq!(preserved, external);
        drop(preserved);
        drop(lease);
        assert_eq!(zero.live_bytes(), 0);
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn effective_version_budget_retains_catalog_and_override_copies() {
        use crate::{
            budget::OperationBudget,
            document::save_options::{PdfVersion, SaveOptions},
            objects::{pdf_number::PdfNumber, pdf_reference::PdfReference},
        };
        let doc = PdfDocument::new_with_version(PdfVersion::V1_3).unwrap();
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_object_id()
            .unwrap();
        let version_id = doc.insert_object(PdfObject::PdfName(PdfName::new("1.6".into())));
        let mut catalog = doc.resolve_object(root_id).unwrap().into_dict().unwrap();
        catalog.insert(
            "Version".into(),
            PdfObject::PdfReference(PdfReference::new(
                version_id.obj_num(),
                version_id.gen_num(),
            )),
        );
        catalog.insert(
            "Custom".into(),
            PdfObject::PdfString(PdfString::new(vec![b'x'; 4096], false)),
        );
        doc.update_object(root_id, PdfObject::PdfDict(catalog.clone()))
            .unwrap();
        let revision = doc.revision();
        let pool = OperationBudget::new(1024 * 1024);
        assert_eq!(
            doc.effective_version_with_budget(&pool).unwrap(),
            doc.effective_version().unwrap()
        );
        assert_eq!(
            doc.effective_version_with_budget(&pool).unwrap(),
            PdfVersion::V1_6
        );
        assert_eq!(pool.live_bytes(), 0);
        let small = OperationBudget::new(pool.peak_bytes() - 1);
        assert!(matches!(
            doc.effective_version_with_budget(&small),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
        assert_eq!(doc.revision(), revision);
        assert_eq!(
            doc.resolve_object(root_id).unwrap(),
            PdfObject::PdfDict(catalog.clone())
        );
        let output =
            super::write_document_with_budget(&doc, SaveOptions::default(), &pool).unwrap();
        assert_eq!(output.as_slice(), doc.save_to_bytes().unwrap());
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        for value in [
            PdfObject::PdfNull,
            PdfObject::PdfName(PdfName::new("1.2".into())),
            PdfObject::PdfName(PdfName::new("unknown".into())),
            PdfObject::PdfNumber(PdfNumber::new(1.6)),
        ] {
            catalog.insert("Version".into(), value);
            doc.update_object(root_id, PdfObject::PdfDict(catalog.clone()))
                .unwrap();
            let plain = doc.effective_version();
            let budgeted = doc.effective_version_with_budget(&pool);
            match (plain, budgeted) {
                (Ok(plain), Ok(budgeted)) => assert_eq!(plain, budgeted),
                (Err(plain), Err(budgeted)) => assert_eq!(plain.to_string(), budgeted.to_string()),
                _ => panic!("effective version behavior differs"),
            }
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn compressed_source_save_budget_preserves_edit_view_and_cache_transfer() {
        use crate::{
            budget::OperationBudget,
            document::save_options::{SaveOptions, StreamCompression},
            objects::{object_id::ObjectId, pdf_number::PdfNumber},
            parser::syntax::ParseMode,
        };
        use std::io::Write;
        let make_source = |encoded: bool| {
            let mut large = vec![b'('; 1];
            large.extend(std::iter::repeat_n(b'x', 65536));
            large.push(b')');
            let objects: [&[u8]; 4] = [
                b"<< /Type /Catalog /Pages 2 0 R /Private 4 0 R >>",
                b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>",
                &large,
            ];
            let mut header = String::new();
            let mut body = Vec::new();
            for (index, object) in objects.iter().enumerate() {
                header.push_str(&format!("{} {} ", index + 1, body.len()));
                body.extend_from_slice(object);
                body.push(b' ');
            }
            let first = header.len();
            let mut payload = header.into_bytes();
            payload.extend_from_slice(&body);
            if encoded {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(&payload).unwrap();
                payload = encoder.finish().unwrap();
            }
            let mut source = b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n".to_vec();
            let stream_offset = source.len() as u32;
            source.extend_from_slice(
                format!(
                    "5 0 obj\n<< /Type /ObjStm /N 4 /First {first} /Length {} {} >>\nstream\n",
                    payload.len(),
                    if encoded { "/Filter /FlateDecode" } else { "" }
                )
                .as_bytes(),
            );
            source.extend_from_slice(&payload);
            source.extend_from_slice(b"\nendstream\nendobj\n");
            let xref_offset = source.len() as u32;
            let mut entries = Vec::new();
            for (kind, field, index) in [
                (0u8, 0u32, 65535u16),
                (2, 5, 0),
                (2, 5, 1),
                (2, 5, 2),
                (2, 5, 3),
                (1, stream_offset, 0),
                (1, xref_offset, 0),
            ] {
                entries.push(kind);
                entries.extend_from_slice(&field.to_be_bytes());
                entries.extend_from_slice(&index.to_be_bytes());
            }
            source.extend_from_slice(format!("6 0 obj\n<< /Type /XRef /Size 7 /Root 1 0 R /W [1 4 2] /Length {} >>\nstream\n", entries.len()).as_bytes());
            source.extend_from_slice(&entries);
            source.extend_from_slice(
                format!("\nendstream\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes(),
            );
            source
        };
        let root = ObjectId::new(1, 0);
        let edited_id = ObjectId::new(4, 0);
        let stream = ObjectId::new(5, 0);
        for encoded in [false, true] {
            let source = make_source(encoded);
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let options = SaveOptions {
                    compression,
                    ..Default::default()
                };
                let doc =
                    PdfDocument::from_bytes_with_mode(source.clone(), ParseMode::Strict).unwrap();
                for id in [root, edited_id, stream] {
                    assert!(!doc.ctx().source_object_is_cached(id));
                }
                let revision = doc.revision();
                let small = OperationBudget::new(8192);
                assert!(matches!(
                    super::write_document_with_budget(&doc, options, &small),
                    Err(crate::error::PdfError::ResourceLimit(_))
                ));
                assert_eq!(small.live_bytes(), 0);
                assert_eq!(doc.revision(), revision);
                for id in [root, edited_id, stream] {
                    assert!(!doc.ctx().source_object_is_cached(id));
                }
                let edited = PdfObject::PdfNumber(PdfNumber::new(17.0));
                doc.update_object(edited_id, edited.clone()).unwrap();
                let revision = doc.revision();
                let ordinary =
                    PdfDocument::from_bytes_with_mode(source.clone(), ParseMode::Strict).unwrap();
                ordinary.update_object(edited_id, edited.clone()).unwrap();
                let expected = ordinary.save_to_bytes_with_options(options).unwrap();
                let pool = OperationBudget::new(2 * 1024 * 1024);
                let output = super::write_document_with_budget(&doc, options, &pool).unwrap();
                assert_eq!(output.as_slice(), expected);
                assert_eq!(pool.live_bytes(), output.capacity());
                for id in [root, edited_id, stream] {
                    assert!(doc.ctx().source_object_is_cached(id));
                }
                assert_eq!(doc.resolve_object(edited_id).unwrap(), edited);
                assert_eq!(doc.revision(), revision);
                let reopened = PdfDocument::from_bytes_with_mode(
                    output.as_slice().to_vec(),
                    ParseMode::Strict,
                )
                .unwrap();
                assert_eq!(reopened.try_page_count().unwrap(), 1);
                assert_eq!(reopened.resolve_object(edited_id).unwrap(), edited);
                drop(output);
                assert_eq!(pool.live_bytes(), 0);
                assert!(doc.ctx().source_object_is_cached(stream));
            }
        }
    }

    #[test]
    fn cold_source_save_budget_refuses_before_large_cache_publication() {
        use crate::{
            budget::OperationBudget,
            document::save_options::{SaveOptions, StreamCompression},
            objects::pdf_stream::PdfStream,
            parser::syntax::ParseMode,
        };
        let fixture = PdfDocument::new().unwrap();
        fixture.add_page(0).unwrap();
        fixture.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        // Unreferenced objects are still materialized by a full rewrite, but
        // opening the Catalog must not eagerly load this payload.
        let payload_id = fixture.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            vec![b'x'; 65536],
        )));
        let source = fixture.save_to_bytes().unwrap();
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let options = SaveOptions {
                compression,
                ..Default::default()
            };
            let expected_doc =
                PdfDocument::from_bytes_with_mode(source.clone(), ParseMode::Strict).unwrap();
            let expected = expected_doc.save_to_bytes_with_options(options).unwrap();
            let doc = PdfDocument::from_bytes_with_mode(source.clone(), ParseMode::Strict).unwrap();
            assert!(!doc.ctx().source_object_is_cached(payload_id));
            let revision = doc.revision();
            let small = OperationBudget::new(8192);
            assert!(matches!(
                super::write_document_with_budget(&doc, options, &small),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(small.live_bytes(), 0);
            assert!(!doc.ctx().source_object_is_cached(payload_id));
            assert_eq!(doc.revision(), revision);
            let pool = OperationBudget::new(2 * 1024 * 1024);
            let output = super::write_document_with_budget(&doc, options, &pool).unwrap();
            assert_eq!(output.as_slice(), expected);
            assert!(doc.ctx().source_object_is_cached(payload_id));
            assert_eq!(pool.live_bytes(), output.capacity());
            assert_eq!(doc.revision(), revision);
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
            // Valid source caches survive quota transfer and ordinary saves.
            assert!(doc.ctx().source_object_is_cached(payload_id));
            assert_eq!(doc.save_to_bytes_with_options(options).unwrap(), expected);
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                b"q Q"
            );
            let output = super::write_document_with_budget(&doc, options, &pool).unwrap();
            assert_eq!(output.as_slice(), expected);
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn document_save_results_share_budget_until_owner_release() {
        use crate::{
            budget::OperationBudget,
            document::save_options::{SaveOptions, StreamCompression},
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q\n".repeat(4096)).unwrap();
        let revision = doc.revision();
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let options = SaveOptions {
                compression,
                ..Default::default()
            };
            let expected = doc.save_to_bytes_with_options(options).unwrap();
            let measure = OperationBudget::new(16 * 1024 * 1024);
            let output = super::write_document_with_budget(&doc, options, &measure).unwrap();
            let capacity = output.capacity();
            let peak = measure.peak_bytes();
            drop(output);
            assert_eq!(measure.live_bytes(), 0);
            let shared = OperationBudget::new(peak + capacity - 1);
            let first = super::write_document_with_budget(&doc, options, &shared).unwrap();
            assert_eq!(first.as_slice(), expected);
            assert_eq!(shared.live_bytes(), capacity);
            assert!(matches!(
                super::write_document_with_budget(&doc, options, &shared),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(shared.live_bytes(), capacity);
            assert_eq!(first.as_slice(), expected);
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.save_to_bytes_with_options(options).unwrap(), expected);
            drop(first);
            assert_eq!(shared.live_bytes(), 0);
            let retry = super::write_document_with_budget(&doc, options, &shared).unwrap();
            assert_eq!(retry.as_slice(), expected);
            drop(retry);
            assert_eq!(shared.live_bytes(), 0);
            let sufficient = OperationBudget::new(peak + capacity);
            let first = super::write_document_with_budget(&doc, options, &sufficient).unwrap();
            let second = super::write_document_with_budget(&doc, options, &sufficient).unwrap();
            assert_eq!(
                sufficient.live_bytes(),
                first.capacity() + second.capacity()
            );
            assert_eq!(first.as_slice(), second.as_slice());
            drop(first);
            assert_eq!(sufficient.live_bytes(), second.capacity());
            drop(second);
            assert_eq!(sufficient.live_bytes(), 0);
        }
    }

    #[test]
    fn budgeted_document_output_matches_save_and_reopens_with_options() {
        use crate::{
            budget::OperationBudget,
            document::save_options::{SaveOptions, StreamCompression},
            error::PdfError,
            parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let revision = doc.revision();
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let options = SaveOptions {
                compression,
                ..Default::default()
            };
            let expected = doc.save_to_bytes_with_options(options).unwrap();
            let pool = OperationBudget::new(16 * 1024 * 1024);
            let output = super::write_document_with_budget(&doc, options, &pool).unwrap();
            assert_eq!(output.as_slice(), expected);
            assert_eq!(pool.live_bytes(), output.capacity());
            let reopened =
                PdfDocument::from_bytes_with_mode(output.as_slice().to_vec(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(reopened.try_page_count().unwrap(), 1);
            assert_eq!(
                reopened
                    .get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                b"q Q"
            );
            let peak = pool.peak_bytes();
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
            let small = OperationBudget::new(peak - 1);
            assert!(matches!(
                super::write_document_with_budget(&doc, options, &small),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(small.live_bytes(), 0);
            assert_eq!(doc.revision(), revision);
            assert!(
                super::write_document_with_budget(
                    &doc,
                    SaveOptions {
                        max_output_bytes: expected.len() - 1,
                        ..options
                    },
                    &pool
                )
                .is_err()
            );
            assert_eq!(pool.live_bytes(), 0);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _output = super::write_document_with_budget(&doc, options, &pool).unwrap();
                panic!("save output owner unwind");
            }));
            assert!(caught.is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }
    #[test]
    fn budgeted_object_output_matches_bytes_and_retains_capacity() {
        use crate::{
            budget::OperationBudget,
            error::PdfError,
            objects::{pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream},
            parser::options::Limits,
        };
        let mut dictionary = PdfDict::default();
        dictionary.insert(
            "Name #".into(),
            PdfObject::PdfName(PdfName::new("中文 #".into())),
        );
        dictionary.insert(
            "String".into(),
            PdfObject::PdfString(PdfString::new((0..=255).collect(), true)),
        );
        dictionary.insert(
            "Number".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"9007199254740993").unwrap()),
        );
        dictionary.insert(
            "Reference".into(),
            PdfObject::PdfReference(PdfReference::new(123, 4)),
        );
        let objects = [
            PdfObject::PdfDict(dictionary.clone()),
            PdfObject::PdfStream(PdfStream::new(dictionary, b"stream payload".to_vec())),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"-.125").unwrap()),
        ];
        for object in objects {
            let mut ordinary = Vec::new();
            super::write_object_limited(&mut ordinary, &object, usize::MAX).unwrap();
            let pool = OperationBudget::new(1024 * 1024);
            let output =
                super::serialize_object_with_budget(&object, Limits::default(), &pool).unwrap();
            assert_eq!(output.as_slice(), ordinary);
            assert_eq!(pool.live_bytes(), output.capacity());
            let peak = pool.peak_bytes();
            let length = output.as_slice().len();
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
            let small = OperationBudget::new(peak - 1);
            assert!(matches!(
                super::serialize_object_with_budget(&object, Limits::default(), &small),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(small.live_bytes(), 0);
            assert!(
                super::serialize_object_with_budget(
                    &object,
                    Limits {
                        max_decoded_bytes: length - 1,
                        ..Default::default()
                    },
                    &pool
                )
                .is_err()
            );
            assert_eq!(pool.live_bytes(), 0);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _output =
                    super::serialize_object_with_budget(&object, Limits::default(), &pool).unwrap();
                panic!("serialized output unwind");
            }));
            assert!(caught.is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
        let invalid = PdfObject::PdfName(PdfName::new("bad\0name".into()));
        let pool = OperationBudget::new(65536);
        assert!(super::serialize_object_with_budget(&invalid, Limits::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
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
    fn serialization_validation_budget_covers_growth_and_error_cleanup() {
        use crate::objects::pdf_stream::PdfStream;
        use crate::{budget::OperationBudget, parser::options::Limits};
        let object = PdfObject::PdfNull;
        let slot = std::mem::size_of::<(&PdfObject, usize)>();
        for maximum in [slot * 3, slot * 3 - 1] {
            let pool = OperationBudget::new(maximum);
            let mut lease = None;
            let mut pending = Vec::new();
            super::reserve_validation_slots(&mut pending, 1, &mut lease, Some(&pool)).unwrap();
            pending.push((&object, 1));
            let result = super::reserve_validation_slots(&mut pending, 1, &mut lease, Some(&pool));
            if maximum == slot * 3 {
                result.unwrap();
                assert_eq!(pool.peak_bytes(), slot * 3);
                assert_eq!(pool.live_bytes(), slot * 2);
            } else {
                assert!(result.is_err());
                assert_eq!(pending.capacity(), 1);
                assert_eq!(pool.live_bytes(), slot);
            }
            assert_eq!(pending.len(), 1);
            drop(pending);
            drop(lease);
            assert_eq!(pool.live_bytes(), 0);
        }
        let pool = OperationBudget::new(slot * 3);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut lease = None;
            let mut pending = Vec::new();
            super::reserve_validation_slots(&mut pending, 2, &mut lease, Some(&pool)).unwrap();
            pending.push((&object, 1));
            panic!("validation table unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        let invalid = PdfObject::PdfArray(PdfArray::from_items(vec![PdfObject::PdfStream(
            PdfStream::new(PdfDict::default(), vec![]),
        )]));
        assert!(
            super::validate_serialization_nodes_with_budget(
                std::iter::once((&invalid, 1)),
                0,
                Limits::default(),
                Some(&pool)
            )
            .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
        let empty = OperationBudget::new(0);
        assert!(
            super::validate_serialization_nodes_with_budget(
                std::iter::once((&object, 1)),
                0,
                Limits::default(),
                Some(&empty)
            )
            .is_err()
        );
        assert_eq!(empty.live_bytes(), 0);
    }

    #[test]
    fn budgeted_trailer_retains_fields_and_charges_owned_copies() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let limits = Limits::default();
        let mut original = PdfDict::default();
        original.insert(
            "Custom".into(),
            PdfObject::PdfString(PdfString::new(vec![b'x'; 4096], false)),
        );
        original.insert(
            "Size".into(),
            PdfObject::PdfString(PdfString::new(vec![b'z'; 8192], false)),
        );
        for key in [
            "Prev",
            "XRefStm",
            "Type",
            "W",
            "Index",
            "Filter",
            "DecodeParms",
            "Length",
        ] {
            original.insert(
                key.into(),
                PdfObject::PdfString(PdfString::new(vec![b'z'; 8192], false)),
            );
        }
        let before = original.clone();
        let mut expected = Vec::new();
        super::write_trailer(&mut expected, &original, u32::MAX, limits).unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        let make_output = |pool: &OperationBudget| super::SerializationOutput {
            bytes: crate::budget::BudgetedBytes::copy_from_slice(pool, &[]).unwrap(),
            limits,
            capacity_limit: usize::MAX,
        };
        let mut output = make_output(&pool);
        super::write_trailer_limited(&mut output, &original, u32::MAX, limits, usize::MAX).unwrap();
        assert_eq!(output.bytes.as_slice(), expected);
        assert!(!expected.windows(8).any(|window| window == b"zzzzzzzz"));
        assert_eq!(pool.live_bytes(), output.bytes.capacity());
        let peak = pool.peak_bytes();
        // Removed xref fields and replaced Size must never be cloned.
        assert!(peak < 32 * 1024);
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        let small = OperationBudget::new(peak - 1);
        let mut output = make_output(&small);
        assert!(matches!(
            super::write_trailer_limited(&mut output, &original, u32::MAX, limits, usize::MAX),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), output.bytes.capacity());
        drop(output);
        assert_eq!(small.live_bytes(), 0);
        assert_eq!(original, before);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut output = make_output(&pool);
            super::write_trailer_limited(&mut output, &original, 0, limits, usize::MAX).unwrap();
            panic!("trailer owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        let zero = OperationBudget::new(0);
        let mut output = make_output(&zero);
        assert!(
            super::write_trailer_limited(&mut output, &original, 1, limits, usize::MAX).is_err()
        );
        assert!(output.bytes.as_slice().is_empty());
        assert_eq!(zero.live_bytes(), 0);
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
    fn temporary_save_stage_panics_preserve_target_and_clean_output() {
        use std::{cell::Cell, io::Write};
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("rzpdf-save-unwind-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let target = directory.join("target.pdf");
        for existing in [false, true] {
            if existing {
                std::fs::write(&target, b"original bytes").unwrap();
            }
            for stage in 0..3 {
                let reached = Cell::new(0);
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::save_file_atomically_with_actions(
                        &target,
                        |file| {
                            reached.set(1);
                            file.write_all(if stage == 0 { b"partial" } else { b"new bytes" })?;
                            if stage == 0 {
                                panic!("injected write unwind");
                            }
                            Ok(())
                        },
                        |file| {
                            reached.set(2);
                            if stage == 1 {
                                panic!("injected sync unwind");
                            }
                            file.sync_all()
                        },
                        |source, destination| {
                            reached.set(3);
                            assert_eq!(destination, target);
                            assert_eq!(std::fs::read(source)?, b"new bytes");
                            panic!("injected replace unwind");
                        },
                    )
                }));
                assert!(caught.is_err());
                assert_eq!(reached.get(), stage + 1);
                if existing {
                    assert_eq!(std::fs::read(&target).unwrap(), b"original bytes");
                } else {
                    assert!(!target.exists());
                }
                assert_eq!(
                    std::fs::read_dir(&directory).unwrap().count(),
                    usize::from(existing)
                );
            }
        }
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn temporary_sync_and_replace_failures_preserve_target_and_clean_output() {
        use std::{cell::Cell, io::Write};
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "rzpdf-sync-rename-fault-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let target = directory.join("target.pdf");
        for existing in [false, true] {
            if existing {
                std::fs::write(&target, b"original bytes").unwrap();
            }
            for fail_sync in [true, false] {
                let synced = Cell::new(false);
                let replaced = Cell::new(false);
                let result = super::save_file_atomically_with_actions(
                    &target,
                    |file| file.write_all(b"new bytes"),
                    |file| {
                        synced.set(true);
                        assert_eq!(file.metadata()?.len(), 9);
                        if fail_sync {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::StorageFull,
                                "injected sync failure",
                            ))
                        } else {
                            file.sync_all()
                        }
                    },
                    |source, destination| {
                        replaced.set(true);
                        assert_eq!(destination, target);
                        assert_eq!(std::fs::read(source)?, b"new bytes");
                        Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "injected replace failure",
                        ))
                    },
                );
                let error = result.unwrap_err().to_string();
                assert!(error.contains(if fail_sync {
                    "sync temporary PDF failed"
                } else {
                    "replace output file failed"
                }));
                assert!(synced.get());
                assert_eq!(replaced.get(), !fail_sync);
                if existing {
                    assert_eq!(std::fs::read(&target).unwrap(), b"original bytes");
                } else {
                    assert!(!target.exists());
                }
                assert_eq!(
                    std::fs::read_dir(&directory).unwrap().count(),
                    usize::from(existing)
                );
            }
        }
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(directory).unwrap();
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
    fn unsupported_encryption_is_rejected_at_open_before_writing() {
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
        assert!(matches!(
            PdfDocument::from_bytes(encrypted),
            Err(crate::error::PdfError::UnsupportedSecurity(_))
        ));
    }

    #[test]
    fn removing_mutable_encrypt_entries_cannot_make_default_save_plaintext() {
        use crate::{
            document::Document,
            error::PdfError,
            security::{EncryptionOptions, SaveSecurity},
        };
        let created = Document::new().unwrap();
        created.add_page(0).unwrap();
        let settings = EncryptionOptions::new("user", "owner").unwrap();
        let bytes = created
            .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
            .unwrap();
        let owner = Document::from_bytes_with_password(bytes, "owner").unwrap();
        let id = owner.ctx().encryption_dictionary_id().unwrap();
        owner.update_object(id, PdfObject::PdfNull).unwrap();
        let mut trailer = owner.ctx().trailer().clone();
        trailer.remove("Encrypt");
        owner.ctx().replace_trailer(trailer).unwrap();
        let bytes = owner.save_to_bytes().unwrap();
        assert!(matches!(
            Document::from_bytes(bytes.clone()),
            Err(PdfError::PasswordRequired)
        ));
        assert_eq!(
            Document::from_bytes_with_password(bytes, "user")
                .unwrap()
                .try_page_count()
                .unwrap(),
            1
        );
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
        // sh is now implemented for types 2/3; keep this diagnostic fixture
        // genuinely unsupported rather than referencing a missing resource.
        let mut shading = PdfDict::default();
        shading.insert(
            "ShadingType".into(),
            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(7.0)),
        );
        let mut shadings = PdfDict::default();
        shadings.insert("S".into(), PdfObject::PdfDict(shading));
        resources.insert("Shading".into(), PdfObject::PdfDict(shadings));
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

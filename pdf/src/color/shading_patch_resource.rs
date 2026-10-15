//! Type6/7 stream resource adapter with explicit retained-storage accounting.
use std::io::{Read, Seek};

use super::shading_patch::{PatchEncoding, PatchMesh, PatchStreamError, decode_patch_stream_kind};
use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_stream::PdfStream,
    pdf_context::PDFContext,
};

fn invalid(message: &str) -> PdfError {
    PdfError::ColorError(format!("patch shading {message}"))
}

pub(crate) fn from_stream<R: Read + Seek>(
    stream: &PdfStream,
    components: usize,
    ctx: &PDFContext<R>,
    retained: usize,
) -> PdfResult<PatchMesh> {
    let required = |key: &str| {
        stream
            .dict()
            .get(key)
            .ok_or_else(|| invalid(&format!("missing {key}")))
    };
    let integer = |key: &str| -> PdfResult<u8> {
        let object = ctx.resolve(required(key)?)?;
        let number = object
            .as_number()
            .ok_or_else(|| invalid("expected integer"))?;
        u8::try_from(number.as_u64_checked()?).map_err(|_| invalid("integer outside byte range"))
    };
    let kind = integer("ShadingType")?;
    if ![6, 7].contains(&kind) {
        return Err(invalid("requires ShadingType 6 or 7"));
    }
    let flag_bits = integer("BitsPerFlag")?;
    let coordinate_bits = integer("BitsPerCoordinate")?;
    let component_bits = integer("BitsPerComponent")?;
    super::shading_patch::patch_record_layout_kind(
        0,
        flag_bits,
        coordinate_bits,
        component_bits,
        components,
        kind == 7,
    )
    .ok_or_else(|| invalid("invalid bit widths or component count"))?;
    let count = components
        .checked_add(2)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| PdfError::ResourceLimit("patch Decode dimension".into()))?;
    let limits = ctx.limits();
    let mut allowance = limits
        .max_decoded_bytes
        .checked_sub(retained)
        .ok_or_else(|| PdfError::ResourceLimit("patch retained shading payload".into()))?;
    if count > limits.max_objects || components.checked_mul(16).is_none_or(|n| n > allowance) {
        return Err(PdfError::ResourceLimit("patch Decode storage".into()));
    }
    let object = ctx.resolve(required("Decode")?)?;
    let array = object
        .as_array()
        .filter(|a| a.len() == count)
        .ok_or_else(|| invalid("Decode dimension"))?;
    let number = |index: usize| -> PdfResult<f64> {
        let object = ctx.resolve(array.get(index).ok_or_else(|| invalid("Decode index"))?)?;
        object
            .as_number()
            .and_then(|n| n.lexeme().parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| invalid("Decode requires finite numbers"))
    };
    let coordinates = [number(0)?, number(1)?, number(2)?, number(3)?];
    let mut decode = Vec::new();
    decode
        .try_reserve_exact(components)
        .map_err(|_| PdfError::ResourceLimit("patch Decode allocation".into()))?;
    for i in 0..components {
        decode.push([number(4 + i * 2)?, number(5 + i * 2)?]);
    }
    allowance = allowance
        .checked_sub(
            decode
                .capacity()
                .checked_mul(16)
                .ok_or_else(|| PdfError::ResourceLimit("patch Decode capacity".into()))?,
        )
        .ok_or_else(|| PdfError::ResourceLimit("patch Decode capacity".into()))?;
    // Legacy stream decoder has separate limits; aggregate slack is checked
    // after it returns. This is not an operation-wide preallocation lease.
    let data = stream.decode_data(ctx)?;
    allowance = allowance
        .checked_sub(data.capacity() - data.len())
        .ok_or_else(|| PdfError::ResourceLimit("patch decoded input capacity".into()))?;
    let mesh = decode_patch_stream_kind(
        &data,
        &PatchEncoding {
            flag_bits,
            coordinate_bits,
            component_bits,
            coordinates,
            components: &decode,
        },
        limits.max_objects,
        allowance,
        kind == 7,
    )
    .map_err(|error| match error {
        PatchStreamError::PatchLimit
        | PatchStreamError::MemoryLimit
        | PatchStreamError::Allocation => PdfError::ResourceLimit(format!("patch mesh {error:?}")),
        _ => invalid(&format!("invalid mesh stream: {error:?}")),
    })?;
    let payload = mesh
        .retained_payload_bytes()
        .and_then(|bytes| bytes.checked_add(retained))
        .ok_or_else(|| PdfError::ResourceLimit("patch retained payload overflow".into()))?;
    if payload > limits.max_decoded_bytes {
        return Err(PdfError::ResourceLimit("patch retained payload".into()));
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    fn stream(source: &str, data: Vec<u8>) -> PdfStream {
        let dictionary = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        PdfStream::new(dictionary, data)
    }
    #[test]
    fn dictionary_adapter_decodes_continuous_records_and_rejects_bad_fields() {
        let doc = PdfDocument::new().unwrap();
        let data = vec![0, 0, 0, 0x2b, 0x12, 0x34, 0x80];
        let source = "<< /ShadingType 6 /BitsPerFlag 2 /BitsPerCoordinate 1 /BitsPerComponent 1 /Decode [0 3 0 3 0 1] >>";
        let mesh = from_stream(&stream(source, data.clone()), 1, doc.ctx(), 0).unwrap();
        assert_eq!(mesh.patches.len(), 2);
        assert_eq!(mesh.patches[1].values, [0.0, 1.0, 1.0, 0.0]);
        let cloned = mesh.clone();
        assert_eq!(cloned.patches[1].values, mesh.patches[1].values);
        assert!(mesh.retained_payload_bytes().unwrap() > 0);
        // Existing retained storage leaves no room for input/configuration.
        // A rejected construction must not poison a later independent retry.
        assert!(matches!(
            from_stream(
                &stream(source, data.clone()),
                1,
                doc.ctx(),
                doc.ctx().limits().max_decoded_bytes
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(from_stream(&stream(source, data.clone()), 1, doc.ctx(), 0).is_ok());
        assert!(
            from_stream(
                &stream(
                    &source.replace("/BitsPerFlag 2", "/BitsPerFlag 2.5"),
                    data.clone()
                ),
                1,
                doc.ctx(),
                0
            )
            .is_err()
        );
        assert!(from_stream(&stream(source, data), 2, doc.ctx(), 0).is_err());
        assert!(matches!(
            from_stream(&stream(source, vec![0; 4]), 1, doc.ctx(), usize::MAX),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}

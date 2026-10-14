//! Typed inline image payloads. Framing uses encoded end markers or exact sample
//! lengths, never a search for the byte sequence `EI` inside binary data.
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::{
        PdfObject,
        pdf_dict::{BudgetedDictBuilder, PdfDict},
        pdf_name::PdfName,
    },
    parser::{options::Limits, syntax::SyntaxParser},
};
use std::{
    io::{Read, Seek},
    ops::Range,
};

#[derive(Debug, Clone)]
pub struct InlineImage {
    dict: PdfDict,
    encoded: Vec<u8>,
    dictionary_range: Option<Range<u64>>,
    data_range: Option<Range<u64>>,
}

impl InlineImage {
    pub fn dictionary(&self) -> &PdfDict {
        &self.dict
    }
    pub fn encoded_data(&self) -> &[u8] {
        &self.encoded
    }
    /// Range after BI and before ID, including dictionary whitespace/comments.
    pub fn dictionary_range(&self) -> Option<Range<u64>> {
        self.dictionary_range.clone()
    }
    /// Byte range in the decoded content stream; absent for newly built images.
    pub fn data_range(&self) -> Option<Range<u64>> {
        self.data_range.clone()
    }

    /// Construct explicitly encoded inline data. Validation uses the same
    /// parser as page content, so a caller cannot inject neighbouring operators.
    pub fn new(dict: PdfDict, encoded: Vec<u8>, limits: Limits) -> PdfResult<Self> {
        let image = Self {
            dict,
            encoded,
            dictionary_range: None,
            data_range: None,
        };
        let bytes = image.to_content_bytes(limits)?;
        let program = super::content_program::ContentProgram::from_decoded_bytes(
            bytes,
            crate::parser::options::ParseOptions {
                limits,
                ..Default::default()
            },
        )?;
        if program.operations().len() != 1
            || program.operations()[0].operator.inline_image().is_none()
        {
            return Err(invalid(
                "inline image construction changed content boundaries",
            ));
        }
        Ok(image)
    }

    pub fn to_content_bytes(&self, limits: Limits) -> PdfResult<Vec<u8>> {
        use std::io::Write;
        if self.encoded.len() > limits.max_raw_stream_bytes
            || self.dict.into_iter().len() > limits.max_objects
        {
            return Err(limit("inline construction input size"));
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BI\n");
        let mut keys: Vec<_> = self.dict.into_iter().collect();
        keys.sort_by(|a, b| a.0.cmp(b.0));
        for (key, value) in keys {
            let name_length = crate::writer::serialized_name_bytes_length(key.as_bytes())?;
            if name_length > limits.max_token_bytes {
                return Err(limit("inline dictionary key size"));
            }
            crate::filter::check_output_size(
                bytes.len(),
                name_length + 1,
                limits.max_raw_stream_bytes,
                "inline dictionary bytes",
            )?;
            crate::writer::write_name(&mut bytes, &PdfName::new(key.clone()))?;
            bytes.write_all(b" ")?;
            let pool = limits.operation_budget();
            let effective = Limits {
                max_decoded_bytes: limits
                    .max_decoded_bytes
                    .min(limits.max_raw_stream_bytes.saturating_sub(bytes.len())),
                ..limits
            };
            let serialized = crate::writer::serialize_object_with_budget(value, effective, &pool)?;
            bytes.write_all(serialized.as_ref())?;
            bytes.write_all(b"\n")?;
            if bytes.len() > limits.max_raw_stream_bytes {
                return Err(limit("inline dictionary bytes"));
            }
        }
        crate::filter::check_output_size(
            bytes.len(),
            self.encoded
                .len()
                .checked_add(8)
                .ok_or_else(|| limit("inline data size"))?,
            limits.max_raw_stream_bytes,
            "inline content bytes",
        )?;
        bytes
            .try_reserve_exact(self.encoded.len() + 8)
            .map_err(|_| limit("inline content allocation"))?;
        bytes.extend_from_slice(b"ID ");
        bytes.extend_from_slice(&self.encoded);
        bytes.extend_from_slice(b"\nEI\n");
        Ok(bytes)
    }

    /// Normalized image-stream dictionary. Abbreviated keys take precedence.
    pub(crate) fn normalized_dict(&self) -> PdfDict {
        let mut dict = PdfDict::default();
        for key in [
            "Width",
            "Height",
            "BitsPerComponent",
            "ColorSpace",
            "Decode",
            "DecodeParms",
            "Filter",
            "ImageMask",
            "Interpolate",
            "Intent",
        ] {
            if let Some(value) = parameter(&self.dict, key) {
                let value = if key == "ColorSpace" || key == "Filter" {
                    expand_names(value)
                } else {
                    value.clone()
                };
                dict.insert(key.into(), value);
            }
        }
        dict
    }
}

fn expand_names(value: &PdfObject) -> PdfObject {
    match value {
        PdfObject::PdfName(name) => PdfObject::PdfName(PdfName::new(
            match name.name() {
                "G" => "DeviceGray",
                "RGB" => "DeviceRGB",
                "CMYK" => "DeviceCMYK",
                "I" => "Indexed",
                "AHx" => "ASCIIHexDecode",
                "A85" => "ASCII85Decode",
                "Fl" => "FlateDecode",
                "LZW" => "LZWDecode",
                "RL" => "RunLengthDecode",
                "CCF" => "CCITTFaxDecode",
                "DCT" => "DCTDecode",
                other => other,
            }
            .into(),
        )),
        PdfObject::PdfArray(array) => {
            PdfObject::PdfArray(crate::objects::pdf_array::PdfArray::from_items(
                array.into_iter().map(expand_names).collect(),
            ))
        }
        _ => value.clone(),
    }
}

fn parameter<'a>(dict: &'a PdfDict, key: &str) -> Option<&'a PdfObject> {
    let short = match key {
        "Width" => "W",
        "Height" => "H",
        "BitsPerComponent" => "BPC",
        "ColorSpace" => "CS",
        "Decode" => "D",
        "DecodeParms" => "DP",
        "Filter" => "F",
        "ImageMask" => "IM",
        "Interpolate" => "i",
        _ => key,
    };
    dict.get(short).or_else(|| dict.get(key))
}
fn invalid(message: &str) -> PdfError {
    PdfError::ContentParseError(message.into())
}
fn limit(message: &str) -> PdfError {
    PdfError::ResourceLimit(message.into())
}
fn whitespace(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

pub(crate) fn read<R: Read + Seek>(
    syntax: &mut SyntaxParser<R>,
    budget: &OperationBudget,
) -> PdfResult<(InlineImage, MemoryLease)> {
    let limits = syntax.limits();
    let mut dict = BudgetedDictBuilder::new(budget, limits.max_objects)?;
    let dictionary_start = syntax.position();
    let dictionary_end = loop {
        let start = syntax.position();
        let word = syntax.get_next_word_with_budget(budget)?;
        if word.is_equal(b"ID") {
            break syntax.position().saturating_sub(2);
        }
        drop(word);
        syntax.set_pos(start)?;
        let (key, key_lease) = syntax.get_content_object_with_budget(budget)?;
        let key = key
            .into_name()
            .ok_or_else(|| invalid("inline image dictionary key must be a name"))?;
        let (owned_key, owned_key_lease) = crate::budget::copy_string(key.name(), budget)?;
        drop(key);
        drop(key_lease);
        let (value, value_lease) = syntax.get_content_object_with_budget(budget)?;
        dict.insert(owned_key, value, owned_key_lease, value_lease)?;
    };
    let (dict, mut retained) = dict.finish()?;
    let width = parameter(&dict, "Width")
        .and_then(PdfObject::as_number)
        .ok_or_else(|| invalid("inline Width missing"))?
        .as_u32_checked()?;
    let height = parameter(&dict, "Height")
        .and_then(PdfObject::as_number)
        .ok_or_else(|| invalid("inline Height missing"))?
        .as_u32_checked()?;
    if width == 0 || height == 0 {
        return Err(invalid("inline dimensions must be positive"));
    }
    if u64::from(width) * u64::from(height) > limits.max_image_pixels {
        return Err(limit("inline image pixels"));
    }
    let mask = parameter(&dict, "ImageMask")
        .map(|v| {
            v.as_bool()
                .map(|v| v.value())
                .ok_or_else(|| invalid("inline ImageMask must be boolean"))
        })
        .transpose()?
        .unwrap_or(false);
    let bits = parameter(&dict, "BitsPerComponent")
        .map(|v| {
            v.as_u32()
                .ok_or_else(|| invalid("inline BitsPerComponent must be integer"))
        })
        .transpose()?
        .unwrap_or(if mask { 1 } else { 8 });
    if !mask && parameter(&dict, "BitsPerComponent").is_none() {
        return Err(invalid("inline BitsPerComponent missing"));
    }
    if !matches!(bits, 1 | 2 | 4 | 8 | 16) || (mask && bits != 1) {
        return Err(invalid("invalid inline BitsPerComponent"));
    }
    if !mask && parameter(&dict, "ColorSpace").is_none() {
        return Err(invalid("inline ColorSpace missing"));
    }
    if let Some(filter) = parameter(&dict, "Filter") {
        let valid = filter.as_name().is_some()
            || filter
                .as_array()
                .is_some_and(|a| a.len() > 0 && a.into_iter().all(|v| v.as_name().is_some()));
        if !valid {
            return Err(invalid(
                "inline Filter must be a name or nonempty name array",
            ));
        }
        let forbidden = |value: &PdfObject| {
            value
                .as_name()
                .is_some_and(|n| matches!(n.name(), "JPXDecode" | "JBIG2Decode"))
        };
        if forbidden(filter)
            || filter
                .as_array()
                .is_some_and(|a| a.into_iter().any(forbidden))
        {
            return Err(PdfError::UnsupportedFeature(
                "JPX/JBIG2 filters are restricted to image XObjects".into(),
            ));
        }
    }
    let separator = syntax.get_next_char()?;
    if !whitespace(separator) {
        return Err(invalid("inline ID requires whitespace"));
    }
    // CRLF is one end-of-line marker. Other whitespace consumes exactly one byte.
    if separator == 13 {
        let pos = syntax.position();
        if syntax.get_next_char()? != 10 {
            syntax.set_pos(pos)?;
        }
    }
    let start = syntax.position();
    let filter = parameter(&dict, "Filter")
        .and_then(|f| f.as_array().and_then(|a| a.get(0)).or(Some(f)))
        .and_then(|f| f.as_name())
        .map(|n| n.name());
    let mut payload = Vec::new();
    let mut payload_lease = budget.reserve(0)?;
    let mut next = |syntax: &mut SyntaxParser<R>| -> PdfResult<u8> {
        if payload.len() >= limits.max_raw_stream_bytes {
            return Err(limit("inline encoded bytes"));
        }
        if payload.len() == payload.capacity() {
            let capacity = payload
                .capacity()
                .saturating_mul(2)
                .max(64)
                .min(limits.max_raw_stream_bytes);
            let mut lease = budget.reserve(capacity)?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(capacity)
                .map_err(|_| limit("inline allocation"))?;
            lease.resize(replacement.capacity())?;
            replacement.append(&mut payload);
            payload = replacement;
            payload_lease = lease;
        }
        let byte = syntax
            .get_next_char()
            .map_err(|_| invalid("truncated inline image data"))?;
        payload.push(byte);
        Ok(byte)
    };
    match filter {
        None => {
            let dimension = |key| {
                parameter(&dict, key)
                    .and_then(PdfObject::as_number)
                    .ok_or_else(|| invalid("inline dimensions missing"))
                    .and_then(|n| n.as_u32_checked())
                    .and_then(|v| {
                        if v == 0 {
                            Err(invalid("inline dimensions must be positive"))
                        } else {
                            Ok(v as usize)
                        }
                    })
            };
            let width = dimension("Width")?;
            let height = dimension("Height")?;
            if (width as u64)
                .checked_mul(height as u64)
                .is_none_or(|n| n > limits.max_image_pixels)
            {
                return Err(limit("inline image pixels"));
            }
            let mask = parameter(&dict, "ImageMask")
                .and_then(PdfObject::as_bool)
                .is_some_and(|v| v.value());
            let bits = parameter(&dict, "BitsPerComponent")
                .and_then(PdfObject::as_u32)
                .unwrap_or(if mask { 1 } else { 8 }) as usize;
            if !matches!(bits, 1 | 2 | 4 | 8 | 16) {
                return Err(invalid("invalid inline BitsPerComponent"));
            }
            let components = if mask {
                1
            } else {
                let cs = parameter(&dict, "ColorSpace")
                    .ok_or_else(|| invalid("inline ColorSpace missing"))?;
                match cs.as_name().map(|n| n.name()).or_else(|| {
                    cs.as_array()
                        .and_then(|a| a.get(0))
                        .and_then(PdfObject::as_name)
                        .map(|n| n.name())
                }) {
                    Some("G" | "DeviceGray" | "I" | "Indexed") => 1,
                    Some("RGB" | "DeviceRGB") => 3,
                    Some("CMYK" | "DeviceCMYK") => 4,
                    _ => {
                        return Err(PdfError::UnsupportedFeature(
                            "unfiltered inline resource color space framing".into(),
                        ));
                    }
                }
            };
            let count = width
                .checked_mul(components)
                .and_then(|n| n.checked_mul(bits))
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .and_then(|n| n.checked_mul(height))
                .filter(|n| *n <= limits.max_raw_stream_bytes)
                .ok_or_else(|| limit("inline sample bytes"))?;
            for _ in 0..count {
                next(syntax)?;
            }
        }
        Some("AHx" | "ASCIIHexDecode") => while next(syntax)? != b'>' {},
        Some("A85" | "ASCII85Decode") => {
            let mut previous = 0;
            loop {
                let b = next(syntax)?;
                if previous == b'~' && b == b'>' {
                    break;
                }
                previous = b;
            }
        }
        Some("RL" | "RunLengthDecode") => loop {
            let control = next(syntax)?;
            if control == 128 {
                break;
            }
            let count = if control < 128 {
                usize::from(control) + 1
            } else {
                1
            };
            for _ in 0..count {
                next(syntax)?;
            }
        },
        Some("Fl" | "FlateDecode") => {
            let mut decoder = flate2::Decompress::new(true);
            let mut output = [0u8; 4096];
            'encoded: loop {
                let byte = next(syntax)?;
                let input = [byte];
                let mut offset = 0;
                loop {
                    let before_in = decoder.total_in();
                    let before_out = decoder.total_out();
                    let status = decoder
                        .decompress(&input[offset..], &mut output, flate2::FlushDecompress::None)
                        .map_err(|_| invalid("invalid inline Flate data"))?;
                    offset += (decoder.total_in() - before_in) as usize;
                    if decoder.total_out() > limits.max_decoded_bytes as u64 {
                        return Err(limit("inline Flate output"));
                    }
                    if status == flate2::Status::StreamEnd {
                        break 'encoded;
                    }
                    if offset == 1 && decoder.total_out() - before_out < output.len() as u64 {
                        break;
                    }
                    if before_in == decoder.total_in() && before_out == decoder.total_out() {
                        return Err(invalid("stalled inline Flate data"));
                    }
                }
            }
        }
        Some("DCT" | "DCTDecode") => {
            if next(syntax)? != 255 || next(syntax)? != 216 {
                return Err(invalid("invalid inline JPEG signature"));
            }
            let mut entropy = false;
            loop {
                let first = next(syntax)?;
                if entropy && first != 255 {
                    continue;
                }
                if first != 255 {
                    return Err(invalid("invalid inline JPEG marker"));
                }
                let mut marker = next(syntax)?;
                while marker == 255 {
                    marker = next(syntax)?;
                }
                if marker == 217 {
                    break;
                }
                if entropy && (marker == 0 || (208..=215).contains(&marker)) {
                    continue;
                }
                if marker == 0 || marker == 216 {
                    return Err(invalid("invalid inline JPEG marker"));
                }
                let length = usize::from(next(syntax)?) * 256 + usize::from(next(syntax)?);
                if length < 2 {
                    return Err(invalid("invalid inline JPEG segment length"));
                }
                for _ in 2..length {
                    next(syntax)?;
                }
                entropy = marker == 218;
            }
        }
        Some("LZW" | "LZWDecode") => {
            let params = parameter(&dict, "DecodeParms")
                .and_then(|p| p.as_array().and_then(|a| a.get(0)).or(Some(p)))
                .and_then(PdfObject::as_dict);
            let early = params
                .and_then(|p| p.get("EarlyChange"))
                .map(|v| v.as_u32())
                .unwrap_or(Some(1))
                .filter(|v| *v <= 1)
                .ok_or_else(|| invalid("invalid inline LZW EarlyChange"))?
                as u16;
            let mut bits = 0u32;
            let mut available = 0;
            let mut width = 9;
            let mut next_code = 258u16;
            let mut previous = false;
            loop {
                while available < width {
                    bits = (bits << 8) | u32::from(next(syntax)?);
                    available += 8;
                }
                available -= width;
                let code = ((bits >> available) & ((1 << width) - 1)) as u16;
                bits &= (1 << available) - 1;
                if code == 257 {
                    break;
                }
                if code == 256 {
                    width = 9;
                    next_code = 258;
                    previous = false;
                    continue;
                }
                if code > next_code || (code == next_code && !previous) {
                    return Err(invalid("invalid inline LZW code"));
                }
                if previous && next_code < 4096 {
                    next_code += 1;
                    if width < 12 && next_code + early > (1 << width) {
                        width += 1;
                    }
                }
                previous = true;
            }
        }
        Some(_) => {
            return Err(PdfError::UnsupportedFeature(
                "inline image encoded boundary for this filter".into(),
            ));
        }
    }
    drop(next);
    let end = syntax.position();
    if !whitespace(syntax.get_next_char()?) {
        return Err(invalid("inline image data must be separated from EI"));
    }
    let end_word = syntax.get_next_word_with_budget(budget)?;
    if !end_word.is_equal(b"EI") {
        return Err(invalid("inline image terminator is not EI"));
    }
    retained.absorb(&mut payload_lease)?;
    let mut model = budget.reserve(std::mem::size_of::<InlineImage>() + 2)?;
    retained.absorb(&mut model)?;
    Ok((
        InlineImage {
            dict,
            encoded: payload,
            dictionary_range: Some(dictionary_start..dictionary_end),
            data_range: Some(start..end),
        },
        retained,
    ))
}

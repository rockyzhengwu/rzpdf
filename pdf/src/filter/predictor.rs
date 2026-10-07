use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn apply_predictor(
    data: Vec<u8>,
    params: Option<&PdfDict>,
    filter_name: &str,
) -> PdfResult<Vec<u8>> {
    let Some(params) = params else {
        return Ok(data);
    };
    let predictor = get_usize(params, "Predictor", 1)? as u32;
    if predictor == 1 {
        return Ok(data);
    }

    let columns = get_usize(params, "Columns", 1)?;
    let colors = get_usize(params, "Colors", 1)?;
    let bits_per_component = get_usize(params, "BitsPerComponent", 8)?;
    if columns == 0 || colors == 0 || !matches!(bits_per_component, 1 | 2 | 4 | 8 | 16) {
        return Err(PdfError::FilterError(format!(
            "{filter_name} invalid predictor dimensions or component depth"
        )));
    }

    match predictor {
        2 => decode_tiff_predictor(data, columns, colors, bits_per_component, filter_name),
        10..=15 => decode_png_predictor(
            data,
            predictor,
            columns,
            colors,
            bits_per_component,
            filter_name,
        ),
        _ => Err(PdfError::FilterError(format!(
            "{filter_name} unsupported predictor value: {predictor}"
        ))),
    }
}

fn get_usize(params: &PdfDict, key: &str, default: usize) -> PdfResult<usize> {
    Ok(match params.get(key) {
        Some(value) => value
            .as_number()
            .ok_or_else(|| PdfError::FilterError(format!("Predictor {key} must be an integer")))?
            .as_u32_checked()? as usize,
        None => default,
    })
}

fn decode_tiff_predictor(
    data: Vec<u8>,
    columns: usize,
    colors: usize,
    bits_per_component: usize,
    filter_name: &str,
) -> PdfResult<Vec<u8>> {
    if bits_per_component != 8 {
        return Err(PdfError::FilterError(format!(
            "{filter_name} TIFF predictor only supports 8-bit components"
        )));
    }
    let row_size = columns
        .checked_mul(colors)
        .ok_or(PdfError::FilterError(format!(
            "{filter_name} TIFF predictor row size overflow"
        )))?;
    if row_size == 0 {
        return Ok(data);
    }
    if data.len() % row_size != 0 {
        return Err(PdfError::FilterError(format!(
            "{filter_name} TIFF predictor data length {} is not a multiple of row size {}",
            data.len(),
            row_size
        )));
    }

    let mut decoded = data;
    for row in decoded.chunks_mut(row_size) {
        for i in colors..row.len() {
            row[i] = row[i].wrapping_add(row[i - colors]);
        }
    }
    Ok(decoded)
}

fn decode_png_predictor(
    mut data: Vec<u8>,
    _predictor: u32,
    columns: usize,
    colors: usize,
    bits_per_component: usize,
    filter_name: &str,
) -> PdfResult<Vec<u8>> {
    let bits_per_pixel = colors
        .checked_mul(bits_per_component)
        .ok_or(PdfError::FilterError(format!(
            "{filter_name} PNG predictor bits-per-pixel overflow"
        )))?;
    let bytes_per_pixel = bits_per_pixel.div_ceil(8);
    let row_size = columns
        .checked_mul(bits_per_pixel)
        .ok_or_else(|| {
            PdfError::FilterError(format!("{filter_name} PNG predictor row size overflow"))
        })?
        .div_ceil(8);
    if row_size == 0 {
        return Ok(Vec::new());
    }
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if row_size >= data.len() {
        return Err(PdfError::FilterError(format!(
            "{filter_name} PNG predictor row exceeds available data"
        )));
    }

    let stride = row_size
        .checked_add(1)
        .ok_or_else(|| PdfError::FilterError(format!("{filter_name} PNG row stride overflow")))?;
    if data.len() % stride != 0 {
        return Err(PdfError::FilterError(format!(
            "{filter_name} PNG predictor row is truncated"
        )));
    }
    // Validate all rows before mutating. The compacted output always trails
    // unread input, so reconstruction can reuse the input allocation.
    for row in data.chunks_exact(stride) {
        if row[0] > 4 {
            return Err(PdfError::FilterError(format!(
                "{filter_name} unknown PNG predictor row filter: {}",
                row[0]
            )));
        }
    }
    let mut cursor = 0;
    let mut start = 0;
    while cursor < data.len() {
        let filter_type = *data.get(cursor).ok_or(PdfError::FilterError(format!(
            "{filter_name} PNG predictor truncated before row filter byte"
        )))?;
        cursor += 1;
        let end = cursor + row_size;
        for i in 0..row_size {
            // Read before writing: start+i < cursor+i for every row.
            let raw = data[cursor + i];
            let left = if i >= bytes_per_pixel {
                data[start + i - bytes_per_pixel]
            } else {
                0
            };
            let up = if start != 0 {
                data[start - row_size + i]
            } else {
                0
            };
            let up_left = if start != 0 && i >= bytes_per_pixel {
                data[start - row_size + i - bytes_per_pixel]
            } else {
                0
            };
            let predicted = match filter_type {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => paeth_predictor(left, up, up_left),
            };
            data[start + i] = raw.wrapping_add(predicted);
        }
        start += row_size;
        cursor = end;
    }
    data.truncate(start);
    Ok(data)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let pa = (p - a as i16).abs();
    let pb = (p - b as i16).abs();
    let pc = (p - c as i16).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::objects::{pdf_dict::PdfDict, pdf_number::PdfNumber, PdfObject};

    use super::apply_predictor;

    #[test]
    fn png_reconstruction_reuses_input_allocation() {
        let mut source = Vec::with_capacity(64);
        source.extend_from_slice(&[0, 10, 20, 30, 2, 1, 2, 3]);
        let pointer = source.as_ptr();
        let capacity = source.capacity();
        let decoded = apply_predictor(source, Some(&png_params(3, 1, 8)), "test").unwrap();
        assert_eq!(decoded, [10, 20, 30, 11, 22, 33]);
        assert_eq!(decoded.as_ptr(), pointer);
        assert_eq!(decoded.capacity(), capacity);
    }

    fn png_params(columns: usize, colors: usize, bits: usize) -> PdfDict {
        let mut params = PdfDict::default();
        for (key, value) in [
            ("Predictor", 15),
            ("Columns", columns),
            ("Colors", colors),
            ("BitsPerComponent", bits),
        ] {
            params.insert(
                key.into(),
                PdfObject::PdfNumber(PdfNumber::new(value as f32)),
            );
        }
        params
    }

    #[test]
    fn png_all_row_filters_use_decoded_previous_rows() {
        let params = png_params(3, 1, 8);
        let encoded = vec![
            0, 10, 20, 30, 1, 1, 1, 1, 2, 4, 5, 6, 3, 1, 2, 3, 4, 1, 2, 3,
        ];
        assert_eq!(
            apply_predictor(encoded, Some(&params), "FlateDecode").unwrap(),
            [10, 20, 30, 1, 2, 3, 5, 7, 9, 3, 7, 11, 4, 9, 14]
        );
        for filter in 0..=4 {
            let encoded = vec![filter, 1, 2, 3];
            let expected = match filter {
                0 | 2 => vec![1, 2, 3],
                1 | 4 => vec![1, 3, 6],
                _ => vec![1, 2, 4],
            };
            assert_eq!(
                apply_predictor(encoded, Some(&params), "FlateDecode").unwrap(),
                expected
            );
        }
        assert!(apply_predictor(vec![5, 1, 2, 3], Some(&params), "FlateDecode").is_err());
        assert!(apply_predictor(vec![0, 1, 2, 3, 0, 1], Some(&params), "FlateDecode").is_err());
    }

    #[test]
    fn png_stride_handles_rgb_sixteen_bit_and_packed_components() {
        for (columns, colors, bits, encoded, expected) in [
            (2, 3, 8, vec![1, 1, 2, 3, 4, 5, 6], vec![1, 2, 3, 5, 7, 9]),
            (2, 1, 16, vec![1, 255, 255, 1, 2], vec![255, 255, 0, 1]),
            (9, 1, 1, vec![1, 128, 128], vec![128, 0]),
        ] {
            assert_eq!(
                apply_predictor(
                    encoded,
                    Some(&png_params(columns, colors, bits)),
                    "FlateDecode"
                )
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn test_tiff_predictor() {
        let mut params = PdfDict::new(HashMap::new());
        params.insert(
            "Predictor".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(2.0)),
        );
        params.insert(
            "Columns".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(3.0)),
        );
        let decoded = apply_predictor(vec![10, 5, 1, 1, 1, 1], Some(&params), "LZWDecode").unwrap();
        assert_eq!(decoded, vec![10, 15, 16, 1, 2, 3]);
    }
}

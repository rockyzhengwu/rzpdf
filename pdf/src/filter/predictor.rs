use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn apply_predictor(data: Vec<u8>, params: Option<&PdfDict>, filter_name: &str) -> PdfResult<Vec<u8>> {
    let Some(params) = params else {
        return Ok(data);
    };
    let predictor = params
        .get("Predictor")
        .and_then(|v| v.as_number())
        .map(|n| n.as_u32_checked())
        .transpose()?
        .unwrap_or(1);
    if predictor == 1 {
        return Ok(data);
    }

    let columns = get_usize(params, "Columns", 1)?;
    let colors = get_usize(params, "Colors", 1)?;
    let bits_per_component = get_usize(params, "BitsPerComponent", 8)?;

    match predictor {
        2 => decode_tiff_predictor(data, columns, colors, bits_per_component, filter_name),
        10..=15 => decode_png_predictor(data, predictor, columns, colors, bits_per_component, filter_name),
        _ => Err(PdfError::FilterError(format!(
            "{filter_name} unsupported predictor value: {predictor}"
        ))),
    }
}

fn get_usize(params: &PdfDict, key: &str, default: usize) -> PdfResult<usize> {
    Ok(match params.get(key).and_then(|v| v.as_number()) {
        Some(v) => v.as_u32_checked()? as usize,
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
    let row_size = columns.checked_mul(colors).ok_or(PdfError::FilterError(format!(
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
    data: Vec<u8>,
    _predictor: u32,
    columns: usize,
    colors: usize,
    bits_per_component: usize,
    filter_name: &str,
) -> PdfResult<Vec<u8>> {
    let bits_per_pixel = colors.checked_mul(bits_per_component).ok_or(PdfError::FilterError(
        format!("{filter_name} PNG predictor bits-per-pixel overflow"),
    ))?;
    let bytes_per_pixel = bits_per_pixel.div_ceil(8);
    let row_size = (columns * bits_per_pixel).div_ceil(8);
    if row_size == 0 {
        return Ok(Vec::new());
    }

    let mut decoded = Vec::with_capacity(data.len());
    let mut previous_row = vec![0_u8; row_size];
    let mut cursor = 0;
    while cursor < data.len() {
        let filter_type = *data.get(cursor).ok_or(PdfError::FilterError(format!(
            "{filter_name} PNG predictor truncated before row filter byte"
        )))?;
        cursor += 1;
        let end = cursor + row_size;
        let row = data.get(cursor..end).ok_or(PdfError::FilterError(format!(
            "{filter_name} PNG predictor row is truncated"
        )))?;
        let current_row = match filter_type {
            0 => row.to_vec(),
            1 => png_sub(row, bytes_per_pixel),
            2 => png_up(row, &previous_row),
            3 => png_average(row, &previous_row, bytes_per_pixel),
            4 => png_paeth(row, &previous_row, bytes_per_pixel),
            _ => {
                return Err(PdfError::FilterError(format!(
                    "{filter_name} unknown PNG predictor row filter: {filter_type}"
                )));
            }
        };
        decoded.extend_from_slice(&current_row);
        previous_row = current_row;
        cursor = end;
    }

    Ok(decoded)
}

fn png_sub(row: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    let mut result = row.to_vec();
    for i in bytes_per_pixel..row.len() {
        result[i] = result[i].wrapping_add(result[i - bytes_per_pixel]);
    }
    result
}

fn png_up(row: &[u8], prev_row: &[u8]) -> Vec<u8> {
    row.iter()
        .zip(prev_row.iter())
        .map(|(&r, &p)| r.wrapping_add(p))
        .collect()
}

fn png_average(row: &[u8], prev_row: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    let mut result = row.to_vec();
    for i in 0..row.len() {
        let left = if i >= bytes_per_pixel {
            result[i - bytes_per_pixel]
        } else {
            0
        };
        let up = prev_row[i];
        result[i] = row[i].wrapping_add(((left as u16 + up as u16) / 2) as u8);
    }
    result
}

fn png_paeth(row: &[u8], prev_row: &[u8], bytes_per_pixel: usize) -> Vec<u8> {
    let mut result = row.to_vec();
    for i in 0..row.len() {
        let left = if i >= bytes_per_pixel {
            result[i - bytes_per_pixel]
        } else {
            0
        };
        let up = prev_row[i];
        let up_left = if i >= bytes_per_pixel {
            prev_row[i - bytes_per_pixel]
        } else {
            0
        };
        result[i] = row[i].wrapping_add(paeth_predictor(left, up, up_left));
    }
    result
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

    use crate::objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber};

    use super::apply_predictor;

    #[test]
    fn test_tiff_predictor() {
        let mut params = PdfDict::new(HashMap::new());
        params.insert("Predictor".to_string(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        params.insert("Columns".to_string(), PdfObject::PdfNumber(PdfNumber::new(3.0)));
        let decoded = apply_predictor(vec![10, 5, 1, 1, 1, 1], Some(&params), "LZWDecode").unwrap();
        assert_eq!(decoded, vec![10, 15, 16, 1, 2, 3]);
    }
}

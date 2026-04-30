//use group4::decode_g4;

use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};
use fax::Color;
use fax::decoder::{decode_g3, decode_g4, pels};

mod bitreader;
mod fax_table;
mod group4;

pub struct Param {
    k: i8,
    columns: u16,
    rows: u16,
    end_of_line: bool,
    encoded_byte_align: bool,
    end_of_block: bool,
    black_is_1: bool,
    damaged_rows_before_error: u16,
}
impl Default for Param {
    fn default() -> Self {
        Param {
            k: 0,
            columns: 1728,
            rows: 0,
            end_of_line: false,
            end_of_block: true,
            encoded_byte_align: false,
            black_is_1: false,
            damaged_rows_before_error: 0,
        }
    }
}
impl Param {
    pub fn try_new(dict: &PdfDict) -> PdfResult<Self> {
        let mut param = Param::default();
        if let Some(k) = dict.get("K") {
            param.k = k
                .as_number()
                .ok_or(PdfError::FilterError("CCITTFaxDecode K must be a number".to_string()))?
                .get_i8();
        }
        if let Some(cl) = dict.get("Columns") {
            param.columns = cl
                .as_u16()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode Columns must be a number".to_string(),
                ))?;
        }
        if let Some(rw) = dict.get("Rows") {
            param.rows = rw
                .as_u16()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode Rows must be a number".to_string(),
                ))?;
        }
        if let Some(black) = dict.get("BlackIs1") {
            param.black_is_1 = black
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode BlackIs1 must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(eol) = dict.get("EndOfLine") {
            param.end_of_line = eol
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EndOfLine must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(eob) = dict.get("EndOfBlock") {
            param.end_of_block = eob
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EndOfBlock must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(aligned) = dict.get("EncodedByteAlign") {
            param.encoded_byte_align = aligned
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EncodedByteAlign must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(rows) = dict.get("DamagedRowsBeforeError") {
            param.damaged_rows_before_error = rows
                .as_u16()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode DamagedRowsBeforeError must be a number".to_string(),
                ))?;
        }
        Ok(param)
    }
}

pub fn ccittfax_decode(buf: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    let p = match params {
        Some(d) => Param::try_new(d)?,
        None => Param::default(),
    };
    let width = p.columns;
    let height = if p.rows > 0 { Some(p.rows) } else { None };
    let mut res = Vec::new();
    if p.k < 0 {
        decode_g4(buf.iter().copied(), width, height, |line| {
            append_line(&mut res, line, p.columns, p.black_is_1);
        })
        .ok_or(PdfError::FilterError(
            "CCITTFaxDecode Group 4 decoding failed".to_string(),
        ))?;
        return Ok(res);
    }
    if p.k == 0 {
        decode_g3(buf.iter().copied(), |line| {
            append_line(&mut res, line, p.columns, p.black_is_1);
        })
        .ok_or(PdfError::FilterError(
            "CCITTFaxDecode Group 3 1D decoding failed".to_string(),
        ))?;
        if let Some(expected_rows) = height {
            let actual_rows = res.len() / p.columns as usize;
            if actual_rows != expected_rows as usize {
                return Err(PdfError::FilterError(format!(
                    "CCITTFaxDecode decoded {actual_rows} rows but expected {expected_rows}"
                )));
            }
        }
        return Ok(res);
    }

    Err(PdfError::FilterError(format!(
        "CCITTFaxDecode mixed Group 3 2D mode (K={}) is not implemented",
        p.k
    )))
}

fn append_line(output: &mut Vec<u8>, line: &[u16], width: u16, black_is_1: bool) {
    output.extend(pels(line, width).map(|color| match (color, black_is_1) {
        (Color::White, false) => 255,
        (Color::Black, false) => 0,
        (Color::White, true) => 0,
        (Color::Black, true) => 255,
    }));
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Read};

    use crate::objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber};

    use super::ccittfax_decode;

    #[test]
    #[ignore = "missing ccittfax fixture in repository"]
    fn test_decode_g4() {
        let mut f = std::fs::File::open("./tests/resources/ccittfax_data").unwrap();
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).unwrap();
        let mut pd = PdfDict::default();
        pd.insert(
            "Columns".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(2869 as f32)),
        );
        pd.insert("K".to_string(), PdfObject::PdfNumber(PdfNumber::new(-1.0)));
        let images = ccittfax_decode(&buffer, Some(&pd)).unwrap();
        assert_eq!(images.len() / 2869, 600);
    }
}

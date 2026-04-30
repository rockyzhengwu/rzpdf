use crate::{
    error::{PdfError, PdfResult},
    filter::predictor::apply_predictor,
    objects::pdf_dict::PdfDict,
};

pub fn lzw_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    let early_change = params
        .and_then(|dict| dict.get("EarlyChange"))
        .and_then(|value| value.as_i32())
        .unwrap_or(1);
    if !(0..=1).contains(&early_change) {
        return Err(PdfError::FilterError(format!(
            "LZWDecode EarlyChange must be 0 or 1, got {early_change}"
        )));
    }

    let mut table = reset_lzw_table();
    let mut result = Vec::new();
    let mut code_size = 9_u8;
    let mut next_code: u16 = 258;
    let mut prev_entry: Option<Vec<u8>> = None;
    let mut reader = BitReader::new(input);

    while let Some(code) = reader.read_bits(code_size) {
        if code == 256 {
            table = reset_lzw_table();
            next_code = 258;
            code_size = 9;
            prev_entry = None;
            continue;
        }
        if code == 257 {
            break;
        }

        let entry = if let Some(Some(entry)) = table.get(code as usize) {
            entry.clone()
        } else if code == next_code {
            let Some(prev) = &prev_entry else {
                return Err(PdfError::FilterError(
                    "LZWDecode encountered a forward reference before any entry".to_string(),
                ));
            };
            let mut new_entry = prev.clone();
            new_entry.push(prev[0]);
            new_entry
        } else {
            return Err(PdfError::FilterError(format!(
                "LZWDecode encountered unknown code: {code}"
            )));
        };

        result.extend_from_slice(&entry);

        if let Some(prev) = prev_entry.take() {
            if next_code < 4096 {
                let mut new_entry = prev;
                new_entry.push(entry[0]);
                table[next_code as usize] = Some(new_entry);
                next_code += 1;
                while code_size < 12 && next_code + early_change as u16 > (1_u16 << code_size) {
                    code_size += 1;
                }
            }
        }
        prev_entry = Some(entry);
    }

    apply_predictor(result, params, "LZWDecode")
}

fn reset_lzw_table() -> Vec<Option<Vec<u8>>> {
    let mut table = vec![None; 4096];
    for value in 0..=255 {
        table[value] = Some(vec![value as u8]);
    }
    table
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_index: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_index: 0 }
    }

    fn read_bits(&mut self, count: u8) -> Option<u16> {
        if count == 0 {
            return Some(0);
        }
        let mut value = 0_u16;
        for _ in 0..count {
            let byte = *self.data.get(self.bit_index / 8)?;
            let shift = 7 - (self.bit_index % 8);
            value = (value << 1) | ((byte >> shift) & 1) as u16;
            self.bit_index += 1;
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::lzw_decode;

    #[test]
    fn test_lzw_decode() {
        let encoded = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        let res = lzw_decode(&encoded, None).unwrap();
        assert_eq!(res, [45, 45, 45, 45, 45, 65, 45, 45, 45, 66]);
    }
}

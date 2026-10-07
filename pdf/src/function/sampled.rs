use std::io::{Cursor, Read, Seek};

use bitstream_io::{BigEndian, BitRead, BitReader};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone, Default)]
pub struct Sampled {
    domain: Vec<f32>,
    range: Vec<f32>,
    size: Vec<usize>,
    bits_per_sample: u8,
    order: u8,
    encode: Vec<f32>,
    decode: Vec<f32>,
    m: usize,
    n: usize,
    samples: Vec<f32>,
    idx_offset: Vec<usize>,
}

impl Sampled {
    pub(super) fn retained_own_bytes(&self) -> PdfResult<usize> {
        super::retained_total(&[
            super::retained_vector_bytes(&self.domain)?,
            super::retained_vector_bytes(&self.range)?,
            super::retained_vector_bytes(&self.size)?,
            super::retained_vector_bytes(&self.encode)?,
            super::retained_vector_bytes(&self.decode)?,
            super::retained_vector_bytes(&self.samples)?,
            super::retained_vector_bytes(&self.idx_offset)?,
        ])
    }
    pub(super) fn dimensions(&self) -> (usize, usize) {
        (self.m, self.n)
    }

    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let resolved = ctx.resolve(obj)?;
        let obj = &resolved;
        let obj = obj.as_stream().ok_or(PdfError::FunctionError(
            "Sampled need a stream ".to_string(),
        ))?;
        let mut function = Sampled::default();
        let mut dict = obj.dict().clone();
        for key in [
            "Domain",
            "Range",
            "Size",
            "BitsPerSample",
            "Order",
            "Encode",
            "Decode",
        ] {
            if let Some(value) = dict.get(key) {
                let mut value = ctx.resolve(value)?;
                if let PdfObject::PdfArray(array) = &mut value {
                    let mut resolved_array = crate::objects::pdf_array::PdfArray::default();
                    for entry in &*array {
                        resolved_array.add_obj(ctx.resolve(entry)?);
                    }
                    *array = resolved_array;
                }
                dict.insert(key.to_string(), value);
            }
        }
        let domain = dict
            .get("Domain")
            .ok_or(PdfError::FunctionError(
                "Sample Function Domain is None".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Sampled function domain is not an array".to_string(),
            ))?;
        for v in domain.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Domain element is not number".to_string(),
            ))?;

            function.domain.push(value);
        }
        function.m = function.domain.len() / 2;

        let range = dict
            .get("Range")
            .ok_or(PdfError::FunctionError("Sampled Range is None".to_string()))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Sampled Function Range is not an array".to_string(),
            ))?;
        for v in range.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Sampled Function range element is not an number".to_string(),
            ))?;
            function.range.push(value);
        }
        function.n = range.len() / 2;
        let size_obj = dict
            .get("Size")
            .ok_or(PdfError::FunctionError(
                "Smapled Function Size param is None".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Smapled Function Size need an aray".to_string(),
            ))?;
        for v in size_obj.into_iter() {
            let value = v.as_number().and_then(|n| n.as_u32_checked().ok()).ok_or(
                PdfError::FunctionError(
                    "Sampled Function Size Value need to be a number".to_string(),
                ),
            )?;
            function.size.push(value as usize);
        }

        if function.domain.is_empty()
            || function.domain.len() % 2 != 0
            || function.range.is_empty()
            || function.range.len() % 2 != 0
            || function.size.len() != function.m
            || function.size.contains(&0)
            || function
                .domain
                .chunks_exact(2)
                .any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0] >= p[1])
            || function
                .range
                .chunks_exact(2)
                .any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0] > p[1])
        {
            return Err(PdfError::FunctionError(
                "Invalid Sampled Domain/Range/Size dimensions".to_string(),
            ));
        }
        let sample_count = function
            .size
            .iter()
            .try_fold(function.n, |total, size| total.checked_mul(*size))
            .ok_or_else(|| PdfError::ResourceLimit("Sampled sample count overflow".to_string()))?;
        let corner_size = 1usize
            .checked_shl(function.m.try_into().unwrap_or(u32::MAX))
            .ok_or_else(|| PdfError::ResourceLimit("Sampled dimensions".to_string()))?;
        if sample_count > ctx.limits().max_objects
            || corner_size > ctx.limits().max_objects
            || sample_count
                .checked_mul(4)
                .is_none_or(|bytes| bytes > ctx.limits().max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "Sampled sample/corner budget".to_string(),
            ));
        }
        let mut idx_offset: Vec<usize> = Vec::with_capacity(corner_size);
        for i in 0..corner_size {
            let mut idx: usize = 0;
            let mut j = function.m - 1;
            let mut t = i;
            while j > 0 {
                let bit = if function.size[j] == 1 {
                    0
                } else {
                    (t >> (function.m - 1)) & 1
                };
                idx = (idx + bit) * (function.size[j - 1]);
                j -= 1;
                t <<= 1;
            }
            let bit = if function.size[0] == 1 {
                0
            } else {
                (t >> (function.m - 1)) & 1
            };
            idx_offset.push((idx + bit) * function.n);
        }
        function.idx_offset = idx_offset;

        let bits_per_smaple = dict
            .get("BitsPerSample")
            .ok_or(PdfError::FunctionError(
                "Sampled Function BitsPerSample is None".to_string(),
            ))?
            .as_number()
            .and_then(|n| n.as_u32_checked().ok())
            .and_then(|n| u8::try_from(n).ok())
            .ok_or(PdfError::FunctionError(
                "Sampled Function BitsPerSample is not number".to_string(),
            ))?;
        if !matches!(bits_per_smaple, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32) {
            return Err(PdfError::FunctionError(format!(
                "Sampled Function BitsPerSample is invalid got  {:?}",
                bits_per_smaple
            )));
        }
        function.bits_per_sample = bits_per_smaple;
        if let Some(o) = dict.get("Order") {
            let ov = o
                .as_number()
                .and_then(|n| n.as_u32_checked().ok())
                .and_then(|n| u8::try_from(n).ok())
                .ok_or(PdfError::FunctionError(
                    "Sampled function order is not number".to_string(),
                ))?;
            function.order = ov;
        } else {
            function.order = 1_u8;
        }
        if function.order != 1 {
            return Err(PdfError::UnsupportedFeature(format!(
                "Sampled interpolation Order {}",
                function.order
            )));
        }
        if let Some(encode) = dict.get("Encode") {
            let enc_array = encode.as_array().ok_or(PdfError::FunctionError(
                "Sampled function Encode is not array".to_string(),
            ))?;
            for v in enc_array.into_iter() {
                let vv = v.as_f32().ok_or(PdfError::FunctionError(
                    "Sampled Function Encode element is not number".to_string(),
                ))?;
                function.encode.push(vv);
            }
        } else {
            for s in function.size.iter() {
                function.encode.push(0.0);
                function.encode.push((s - 1) as f32);
            }
        }
        if let Some(decode) = dict.get("Decode") {
            let dec_array = decode.as_array().ok_or(PdfError::FunctionError(
                "Sampled function Decode is not array".to_string(),
            ))?;
            for v in dec_array.into_iter() {
                let vv = v.as_f32().ok_or(PdfError::FunctionError(
                    "Sampled Function Decode element is not number".to_string(),
                ))?;
                function.decode.push(vv);
            }
        } else {
            function.decode = function.range.clone();
        }
        if function.encode.len() != function.m * 2
            || function.decode.len() != function.n * 2
            || function
                .encode
                .iter()
                .chain(function.decode.iter())
                .any(|v| !v.is_finite())
        {
            return Err(PdfError::FunctionError(
                "Invalid Sampled Encode/Decode dimensions".to_string(),
            ));
        }
        let mut samples = Vec::new();
        let bytes = obj.decode_data(ctx)?;
        let required_bytes = sample_count
            .checked_mul(usize::from(function.bits_per_sample))
            .and_then(|bits| bits.checked_add(7))
            .map(|bits| bits / 8)
            .ok_or_else(|| PdfError::ResourceLimit("Sampled bit count overflow".to_string()))?;
        if bytes.len() < required_bytes {
            return Err(PdfError::FunctionError(
                "Truncated Sampled sample data".to_string(),
            ));
        }
        let mut bitreader = BitReader::endian(Cursor::new(bytes), BigEndian);
        let sample_mul = 1.0 / (2.0_f32.powf(function.bits_per_sample as f32) as f32 - 1.0);
        for _ in 0..sample_count {
            let v = bitreader
                .read_var::<u32>(function.bits_per_sample as u32)
                .map_err(|_| PdfError::FunctionError("Truncated Sampled bits".to_string()))?;
            samples.push((v as f32) * sample_mul);
        }
        function.samples = samples;
        Ok(function)
    }

    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        if self.m == 0
            || self.n == 0
            || inputs.len() != self.m
            || inputs.iter().any(|v| !v.is_finite())
        {
            return Err(PdfError::FunctionError(
                "Invalid Sampled inputs".to_string(),
            ));
        }
        let mut e: Vec<usize> = Vec::with_capacity(self.m);
        let mut efrac0: Vec<f32> = Vec::with_capacity(self.m);
        let mut efrac1: Vec<f32> = Vec::with_capacity(self.m);

        let normal = normalize(inputs, self.domain.as_slice())?;
        for (i, v) in normal.iter().enumerate() {
            let mut x = interpolate(
                v,
                self.domain[2 * i],
                self.domain[2 * i + 1],
                self.encode[2 * i],
                self.encode[2 * i + 1],
            );
            let s = self.size[i] as f32;
            if x > s {
                x = s;
            }
            if x < 0.0 {
                x = 0.0;
            } else if x > (self.size[i] - 1) as f32 {
                x = (self.size[i] - 1) as f32;
            }
            let floor = x.floor() as usize;
            if floor == self.size[i] - 1 && self.size[i] > 1 {
                e.push(self.size[i] - 2);
            } else {
                e.push(floor);
            }
            let fraction = x - e.last().copied().unwrap_or(0) as f32;
            efrac0.push(1.0 - fraction);
            efrac1.push(fraction);
        }
        let mut idx0: usize = 0;
        let mut k = self.m - 1;
        while k > 0 {
            idx0 = (idx0 + e[k]) * self.size[k - 1];
            k -= 1;
        }
        idx0 = (idx0 + e[0]) * self.n;
        let mut out = Vec::new();
        for i in 0..self.n {
            let mut values: Vec<f32> = Vec::with_capacity(1 << self.m);
            for j in 0..(1 << self.m) {
                values.push(
                    *self
                        .samples
                        .get(idx0 + self.idx_offset[j] + i)
                        .ok_or_else(|| {
                            PdfError::FunctionError("Sampled index exceeds sample data".to_string())
                        })?,
                );
            }
            let mut j = 0;
            let mut t = 1 << self.m;
            while j < self.m {
                let mut k = 0;
                while k < t {
                    values[k >> 1] = efrac0[j] * values[k] + efrac1[j] * values[k + 1];
                    k += 2;
                }
                j += 1;
                t >>= 1;
            }
            let mut o =
                values[0] * (self.decode[2 * i + 1] - self.decode[2 * i]) + self.decode[2 * i];
            if !o.is_finite() {
                return Err(PdfError::FunctionError(
                    "Sampled result is non-finite".to_string(),
                ));
            }
            if o < self.range[2 * i] {
                o = self.range[2 * i];
            }
            if o > self.range[2 * i + 1] {
                o = self.range[2 * i + 1];
            }
            out.push(o);
        }
        Ok(out)
    }
}

fn interpolate(x: &f32, x_min: f32, x_max: f32, y_min: f32, y_max: f32) -> f32 {
    y_min + (x - x_min) * ((y_max - y_min) / (x_max - x_min))
}

fn normalize(values: &[f32], limits: &[f32]) -> PdfResult<Vec<f32>> {
    if values.len() * 2 != limits.len() {
        return Err(PdfError::FunctionError(
            "SampleFunction normalize values size errror".to_string(),
        ));
    }
    let mut res = Vec::new();
    for (i, v) in values.iter().enumerate() {
        let lowerbound: f32 = limits
            .get(i * 2)
            .ok_or(PdfError::FunctionError(
                "SampleFunction normailize get lowerbound error".to_string(),
            ))?
            .to_owned();
        let upperbound: f32 = limits
            .get(i * 2 + 1)
            .ok_or(PdfError::FunctionError(
                "SampleFunction normailize get upperbound error".to_string(),
            ))?
            .to_owned();
        let normal = v.max(lowerbound).min(upperbound);
        res.push(normal);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::collections::HashMap;

    #[test]
    fn sampled_interpolation_unsigned_bits_and_invalid_dimensions() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let build = |parameters: &str, bytes: Vec<u8>| {
            let mut syntax = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(parameters.as_bytes().to_vec())).unwrap(),
            );
            let dict = syntax.get_object().unwrap().into_dict().unwrap();
            Sampled::try_new(&PdfObject::PdfStream(PdfStream::new(dict, bytes)), &ctx)
        };
        let f = build(
            "<< /Domain [2 4] /Range [0 1] /Size [2] /BitsPerSample 8 >>",
            vec![0, 255],
        )
        .unwrap();
        assert_eq!(f.eval(&[2.0]).unwrap(), vec![0.0]);
        assert_eq!(f.eval(&[3.0]).unwrap(), vec![0.5]);
        assert_eq!(f.eval(&[4.0]).unwrap(), vec![1.0]);
        assert_eq!(f.eval(&[9.0]).unwrap(), vec![1.0]);
        let reverse = build(
            "<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Encode [1 0] >>",
            vec![0, 255],
        )
        .unwrap();
        assert_eq!(reverse.eval(&[0.0]).unwrap(), vec![1.0]);
        assert_eq!(reverse.eval(&[1.0]).unwrap(), vec![0.0]);
        let multidimensional = build(
            "<< /Domain [0 1 0 1] /Range [0 1] /Size [2 2] /BitsPerSample 8 >>",
            vec![0, 255, 255, 0],
        )
        .unwrap();
        assert_eq!(multidimensional.eval(&[0.0, 0.0]).unwrap(), vec![0.0]);
        assert_eq!(multidimensional.eval(&[1.0, 0.0]).unwrap(), vec![1.0]);
        assert_eq!(multidimensional.eval(&[0.0, 1.0]).unwrap(), vec![1.0]);
        assert_eq!(multidimensional.eval(&[1.0, 1.0]).unwrap(), vec![0.0]);
        assert_eq!(multidimensional.eval(&[0.5, 0.5]).unwrap(), vec![0.5]);
        let mut syntax = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 >>".to_vec(),
            ))
            .unwrap(),
        );
        let mut dict = syntax.get_object().unwrap().into_dict().unwrap();
        for key in ["Domain", "Range", "Size", "BitsPerSample"] {
            let mut value = dict.get(key).unwrap().clone();
            if let PdfObject::PdfArray(array) = &mut value {
                let mut referenced = crate::objects::pdf_array::PdfArray::default();
                for entry in &*array {
                    let id = ctx.insert_object(entry.clone());
                    referenced.add_obj(PdfObject::PdfReference(
                        crate::objects::pdf_reference::PdfReference::new(
                            id.obj_num(),
                            id.gen_num(),
                        ),
                    ));
                }
                *array = referenced;
            }
            let id = ctx.insert_object(value);
            dict.insert(
                key.into(),
                PdfObject::PdfReference(crate::objects::pdf_reference::PdfReference::new(
                    id.obj_num(),
                    id.gen_num(),
                )),
            );
        }
        let referenced = Sampled::try_new(
            &PdfObject::PdfStream(PdfStream::new(dict, vec![0, 255])),
            &ctx,
        )
        .unwrap();
        assert_eq!(referenced.eval(&[0.5]).unwrap(), vec![0.5]);
        assert!(f.eval(&[]).is_err());
        assert!(f.eval(&[f32::NAN]).is_err());
        assert!(Sampled::default().eval(&[]).is_err());
        let f = build(
            "<< /Domain [0 1] /Range [0 1] /Size [1] /BitsPerSample 32 >>",
            vec![255; 4],
        )
        .unwrap();
        assert_eq!(f.eval(&[0.5]).unwrap(), vec![1.0]);
        for parameters in [
            "<< /Domain [] /Range [0 1] /Size [] /BitsPerSample 8 >>",
            "<< /Domain [0 1] /Range [0 1] /Size [0] /BitsPerSample 8 >>",
            "<< /Domain [0 1] /Range [0 1] /Size [4294967295] /BitsPerSample 8 >>",
            "<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 264 >>",
            "<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Encode [0] >>",
            "<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 /Order 3 >>",
        ] {
            assert!(build(parameters, vec![0, 255]).is_err(), "{parameters}");
        }
        assert!(
            build(
                "<< /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 >>",
                vec![0]
            )
            .is_err()
        );
    }
}

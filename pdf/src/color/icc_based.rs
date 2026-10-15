use std::io::{Read, Seek};

use crate::{
    color::{colorspace::ColorSpace, icc_profile::ICCProfile},
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

use super::device_cmyk::DeviceCmyk;
use super::device_gray::DeviceGray;
use super::device_rgb::DeviceRgb;
use super::value::{ColorRgb, ColorValue};

#[derive(Debug, Clone)]
pub struct IccBased {
    n: u8,
    alternate: Box<ColorSpace>,
    range: Vec<f32>,
    profile: Option<ICCProfile>,
}

impl Default for IccBased {
    fn default() -> Self {
        IccBased {
            n: 1,
            alternate: Box::new(ColorSpace::DeviceGray(DeviceGray::new())),
            range: Vec::new(),
            profile: None,
        }
    }
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
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn icc_component_count_alternate_and_range_are_checked() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let n = dict
                .get("N")
                .map(|v| ctx.resolve(v).unwrap())
                .and_then(|v| v.as_u32());
            let profile = if n == Some(1) {
                ICCProfile::gray_srgb().unwrap()
            } else {
                ICCProfile::srgb()
            };
            IccBased::try_new(
                &PdfObject::PdfStream(PdfStream::new(dict, profile.to_bytes().unwrap())),
                &ctx,
            )
        };
        let color = parse("<< /N 3 /Range [0 1 0 1 0 1] >>").unwrap();
        assert!(color.rgb(&color.default_value().unwrap()).is_ok());
        assert!(color.rgb(&ColorValue::new(vec![])).is_err());
        assert!(color.rgb(&ColorValue::new(vec![f32::NAN; 3])).is_err());
        for source in [
            "<< /N 1.5 >>",
            "<< /N -1 >>",
            "<< /N 2 >>",
            "<< /N 3 /Alternate /DeviceGray >>",
            "<< /N 1 /Alternate /Pattern >>",
            "<< /N 1 /Range [1 0] >>",
            "<< /N 1 /Range [0 /bad] >>",
            "<< /N 1 /Range [0] >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
        let id = ctx.insert_object(PdfObject::PdfNumber(
            crate::objects::pdf_number::PdfNumber::new_from_lexeme(b"1").unwrap(),
        ));
        assert!(parse(&format!("<< /N {} {} R >>", id.obj_num(), id.gen_num())).is_ok());
    }
}

impl IccBased {
    pub(crate) fn retained_profile_bytes(&self) -> PdfResult<usize> {
        self.alternate
            .retained_profile_bytes()?
            .checked_add(
                self.profile
                    .as_ref()
                    .map_or(0, ICCProfile::retained_source_bytes),
            )
            .ok_or_else(|| PdfError::ResourceLimit("ICC retained profile bytes".into()))
    }
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let resolved = ctx.resolve(obj)?;
        let color_stream =
            match &resolved {
                PdfObject::PdfArray(array) => {
                    if array.len() != 2 {
                        return Err(PdfError::ColorError(
                            "IccBased Color array need 2 param at least".to_string(),
                        ));
                    }
                    let cd = ctx
                        .resolve(array.get(1).ok_or_else(|| {
                            PdfError::ColorError("Missing ICCBased stream".into())
                        })?)?
                        .into_stream()
                        .ok_or(PdfError::ColorError(format!("IccBased need Streamgot",)))?;
                    cd
                }
                PdfObject::PdfStream(s) => s.clone(),
                _ => {
                    return Err(PdfError::ColorError("Bad IccBased Color".to_string()));
                }
            };

        let mut color = IccBased::default();
        if let Some(n) = color_stream.dict().get("N") {
            let n = ctx.resolve(n)?;
            let nv =
                n.as_number()
                    .and_then(|n| n.as_u32_checked().ok())
                    .ok_or(PdfError::ColorError(format!(
                        "IccBased N is not a number:{:?}",
                        n
                    )))?;
            if !matches!(nv, 1 | 3 | 4) {
                return Err(PdfError::ColorError(format!(
                    "IccBased N is must 1,3,or 4 got: {:?}",
                    nv
                )));
            }
            color.n = nv as u8;
        } else {
            return Err(PdfError::ColorError(format!(
                "IccBased color need a N parameter "
            )));
        }
        if let Some(alt) = color_stream.dict().get("Alternate") {
            let altc = ColorSpace::try_new(alt, ctx)?;
            if matches!(
                altc,
                ColorSpace::Pattern(_)
                    | ColorSpace::Indexed(_)
                    | ColorSpace::Separation(_)
                    | ColorSpace::DeviceN(_)
            ) || altc.number_of_components()? != usize::from(color.n)
            {
                return Err(PdfError::ColorError(
                    "ICCBased Alternate has invalid type or component count".into(),
                ));
            }
            color.alternate = Box::new(altc);
        } else {
            match color.n {
                1 => {
                    color.alternate = Box::new(ColorSpace::DeviceGray(DeviceGray::new()));
                }
                3 => {
                    color.alternate = Box::new(ColorSpace::DeviceRgb(DeviceRgb::new()));
                }
                4 => {
                    color.alternate = Box::new(ColorSpace::DeviceCmyk(DeviceCmyk::new()));
                }
                _ => { // donothing}
                }
            }
        }
        match color_stream.dict().get("Range") {
            Some(r) => {
                let r = ctx.resolve(r)?;
                let ra = r.as_array().ok_or(PdfError::ColorError(format!(
                    "IccBased Color need an array got :{:?}",
                    r
                )))?;
                if ra.len() != usize::from(color.n) * 2 {
                    return Err(PdfError::ColorError(format!(
                        "IccBased range array element is not valid got:{:?}",
                        ra.len()
                    )));
                }
                for i in 0..color.n * 2 {
                    let entry = ctx.resolve(ra.get(i as usize).ok_or_else(|| {
                        PdfError::ColorError("Truncated ICCBased Range".into())
                    })?)?;
                    let v = entry.as_f32().ok_or_else(|| {
                        PdfError::ColorError("ICCBased Range requires numeric values".to_string())
                    })?;
                    if !v.is_finite() {
                        return Err(PdfError::ColorError(
                            "ICCBased Range must be finite".to_string(),
                        ));
                    }
                    color.range.push(v);
                }
            }
            None => {
                for _ in 0..color.n {
                    color.range.push(0.0);
                    color.range.push(1.0);
                }
            }
        }

        if color.range.chunks_exact(2).any(|pair| pair[0] > pair[1]) {
            return Err(PdfError::ColorError(
                "ICCBased Range pairs must be ordered".into(),
            ));
        }
        let data = color_stream.decode_data(ctx)?;
        let profile = ICCProfile::try_new_with_limits(&data, ctx.limits())?;
        if profile.number_of_components() != usize::from(color.n) {
            return Err(PdfError::ColorError(
                "ICCBased N disagrees with embedded ICC profile".into(),
            ));
        }
        color.profile = Some(profile);
        Ok(color)
    }

    pub fn default_value(&self) -> PdfResult<ColorValue> {
        let default = match self.n {
            1 => ColorValue::new(vec![0.0]),
            3 => ColorValue::new(vec![0.0, 0.0, 0.0]),
            4 => ColorValue::new(vec![0.0, 0.0, 0.0, 0.0]),
            _ => {
                return Err(PdfError::ColorError(
                    "ICCBased N must be 1, 3, or 4".to_string(),
                ));
            }
        };
        Ok(ColorValue::new(
            default
                .values()
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let range = self.range.get(i * 2..i * 2 + 2).unwrap_or(&[0., 1.]);
                    v.clamp(range[0], range[1])
                })
                .collect(),
        ))
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        self.rgb_with_context(value, &super::conversion::ColorConversionContext::default())
    }
    pub fn rgb_with_context(
        &self,
        value: &ColorValue,
        context: &super::conversion::ColorConversionContext,
    ) -> PdfResult<ColorRgb> {
        if value.value_size() != usize::from(self.n)
            || value.values().iter().any(|v| !v.is_finite())
        {
            return Err(PdfError::ColorError(
                "ICCBased requires finite components matching N".into(),
            ));
        }
        let mut clipped = [0f32; 4];
        for (i, v) in value.values().iter().enumerate() {
            let range = self.range.get(i * 2..i * 2 + 2).unwrap_or(&[0., 1.]);
            clipped[i] = v.clamp(range[0], range[1]);
        }
        if let Some(profile) = &self.profile {
            match profile.convert(&clipped[..usize::from(self.n)], context) {
                Ok(color) => return Ok(color),
                Err(PdfError::UnsupportedFeature(_))
                    if context.icc_fallback == super::conversion::IccFallback::Alternate => {}
                Err(error) => return Err(error),
            }
        } else if context.icc_fallback == super::conversion::IccFallback::Reject {
            return Err(PdfError::ColorError(
                "ICCBased has no usable profile; Alternate requires explicit fallback policy"
                    .into(),
            ));
        }
        self.alternate.rgb_with_context(
            &ColorValue::new(clipped[..usize::from(self.n)].to_vec()),
            context,
        )
    }

    pub fn profile(&self) -> Option<&ICCProfile> {
        self.profile.as_ref()
    }

    pub fn number_of_components(&self) -> usize {
        self.n as usize
    }
    pub fn range(&self) -> &[f32] {
        self.range.as_slice()
    }
}

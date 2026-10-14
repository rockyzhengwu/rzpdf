use std::io::{Read, Seek};

use crate::{
    color::colorspace::ColorSpace,
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub struct PdfImage {
    name: String,
    width: u32,
    height: u32,
    colorspace: ColorSpace,
    data: Vec<u8>,
    is_mask: bool,
    is_inline: bool,
    pixel_count: usize,
    max_rgba_bytes: usize,
    soft_mask: Option<Vec<u8>>,
    // Non-device samples retain packed precision until color conversion.
    color_sample_decode: Option<(usize, Vec<f64>)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::device_rgb::DeviceRgb;

    #[test]
    fn nondevice_image_decode_and_retained_budget_boundaries() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{
                options::{Limits, ParseOptions},
                syntax::SyntaxParser,
            },
        };
        use std::io::Cursor;
        let make_context = |limit| {
            PDFContext::<Cursor<Vec<u8>>>::new_without_source(
                PdfDict::default(),
                ParseOptions {
                    limits: Limits {
                        max_decoded_bytes: limit,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
        };
        let image = |decode: &str, bytes: Vec<u8>| {
            let source = format!(
                "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/CalGray << /WhitePoint [0.95047 1 1.08883] >>] {decode} >>"
            );
            let dict =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap()
                    .into_dict()
                    .unwrap();
            PdfObject::PdfStream(PdfStream::new(dict, bytes))
        };
        let ctx = make_context(1024);
        for decode in [
            "/Decode [0]",
            "/Decode [0 1 0 1]",
            "/Decode /Bad",
            "/Decode [0 /Bad]",
        ] {
            assert!(
                matches!(
                    PdfImage::try_new("invalid".into(), &image(decode, vec![128]), &ctx),
                    Err(PdfError::ObjectError(_))
                ),
                "{decode}"
            );
        }
        for bytes in [vec![], vec![0, 1]] {
            assert!(matches!(
                PdfImage::try_new("invalid".into(), &image("", bytes), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        let mut valid =
            PdfImage::try_new("valid".into(), &image("/Decode null", vec![128]), &ctx).unwrap();
        let endpoint_capacity = valid.color_sample_decode.as_ref().unwrap().1.capacity();
        assert_eq!(
            valid.retained_sample_bytes().unwrap(),
            1 + endpoint_capacity * std::mem::size_of::<f64>()
        );
        assert_eq!(valid.rgba_buffer().unwrap().len(), 4);
        valid.max_rgba_bytes = 3;
        assert!(matches!(
            valid.rgba_buffer(),
            Err(PdfError::ResourceLimit(_))
        ));
        let constrained = make_context(15);
        assert!(
            matches!(PdfImage::try_new("endpoint-budget".into(), &image("", vec![128]), &constrained), Err(PdfError::ResourceLimit(message)) if message == "image Decode endpoint budget")
        );
        let exact = make_context(16);
        assert!(PdfImage::try_new("endpoint-budget".into(), &image("", vec![128]), &exact).is_ok());
    }

    #[test]
    fn calibrated_images_keep_packed_samples_and_lab_decode_ranges() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (space, bits, bytes, decode, expected_values) in [
            (
                "[/Separation /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
                16,
                vec![0, 0, 255, 255],
                "/Decode [1 0]",
                vec![vec![1.0], vec![0.0]],
            ),
            (
                "[/Lab << /WhitePoint [0.95047 1 1.08883] /Range [0 0 0 0] >>]",
                8,
                vec![0, 0, 0, 255, 0, 0],
                "",
                vec![vec![0.0, 0.0, 0.0], vec![100.0, 0.0, 0.0]],
            ),
            (
                "[/CalGray << /WhitePoint [0.95047 1 1.08883] >>]",
                16,
                vec![0x80, 0, 255, 255],
                "/Decode [1 0]",
                vec![vec![1.0 - 32768.0 / 65535.0], vec![0.0]],
            ),
            (
                "[/CalRGB << /WhitePoint [0.95047 1 1.08883] >>]",
                2,
                vec![0x1c, 0xe0],
                "",
                vec![vec![0.0, 1.0 / 3.0, 1.0], vec![1.0, 2.0 / 3.0, 0.0]],
            ),
        ] {
            let height = if bits == 2 { 2 } else { 1 };
            let width = if bits == 2 { 1 } else { 2 };
            let source = format!(
                "<< /Width {width} /Height {height} /BitsPerComponent {bits} /ColorSpace {space} {decode} >>"
            );
            let dict =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap()
                    .into_dict()
                    .unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes.clone()));
            let image = PdfImage::try_new("calibrated".into(), &object, &ctx).unwrap();
            assert_eq!(image.data(), bytes);
            let expected = expected_values
                .into_iter()
                .flat_map(|values| {
                    let rgb = image
                        .colorspace
                        .rgb(&crate::color::value::ColorValue::new(values))
                        .unwrap();
                    [rgb.r(), rgb.g(), rgb.b(), 255]
                })
                .collect::<Vec<_>>();
            assert_eq!(image.rgba_buffer().unwrap(), expected, "{space}");
        }
    }

    #[test]
    fn indexed_images_preserve_indices_across_depths_rows_and_decode() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let palette = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 255],
        ];
        for (bits, width, height, bytes, decode, expected) in [
            (1, 3, 2, vec![0x40, 0xa0], "", vec![0, 1, 0, 1, 0, 1]),
            (2, 4, 1, vec![0x1b], "", vec![0, 1, 2, 3]),
            (4, 4, 1, vec![0x01, 0x2f], "", vec![0, 1, 2, 15]),
            (8, 4, 1, vec![0, 1, 2, 255], "", vec![0, 1, 2, 255]),
            // Repeated indices reuse the same converted palette entry across rows.
            (
                8,
                3,
                2,
                vec![2, 2, 255, 2, 255, 255],
                "",
                vec![2, 2, 255, 2, 255, 255],
            ),
            (
                16,
                4,
                1,
                vec![0, 0, 0, 1, 0, 2, 255, 255],
                "",
                vec![0, 1, 2, 255],
            ),
            (
                16,
                4,
                1,
                vec![0, 0, 0x55, 0x55, 0xaa, 0xaa, 255, 255],
                "/Decode [3 0]",
                vec![3, 2, 1, 0],
            ),
            (
                8,
                4,
                1,
                vec![0, 85, 170, 255],
                "/Decode [3 0]",
                vec![3, 2, 1, 0],
            ),
            (2, 4, 1, vec![0x1b], "/Decode [-2 4]", vec![0, 0, 2, 4]),
        ] {
            let dict = parse(&format!("<< /Width {width} /Height {height} /BitsPerComponent {bits} /ColorSpace [/Indexed /DeviceRGB 3 <FF000000FF000000FFFFFFFF>] {decode} >>")).into_dict().unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes));
            let image = PdfImage::try_new("palette".into(), &object, &ctx).unwrap();
            assert_eq!(image.data, expected, "bits={bits}, decode={decode}");
            let expected_rgba = expected
                .iter()
                .flat_map(|index| palette[(*index as usize).min(3)])
                .collect::<Vec<_>>();
            assert_eq!(image.rgba_buffer().unwrap(), expected_rgba);
        }
        for (bits, decode, bytes) in [(2, "/Decode [0]", vec![0]), (2, "", vec![])] {
            let dict = parse(&format!("<< /Width 1 /Height 1 /BitsPerComponent {bits} /ColorSpace [/Indexed /DeviceRGB 0 <FF0000>] {decode} >>")).into_dict().unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes));
            assert!(matches!(
                PdfImage::try_new("palette".into(), &object, &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
    }

    #[test]
    fn stencil_samples_use_current_color_and_reverse_decode() {
        use crate::{
            color::value::ColorRgb,
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::options::ParseOptions,
            parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (decode, expected) in [
            ("", vec![255, 0, 255, 0, 255, 0]),
            ("/Decode [1 0]", vec![0, 255, 0, 255, 0, 255]),
        ] {
            let dict = parse(&format!(
                "<< /Subtype /Image /Width 3 /Height 2 /ImageMask true {decode} >>"
            ));
            let object = PdfObject::PdfStream(PdfStream::new(
                dict.as_dict().unwrap().clone(),
                vec![0x40, 0xa0],
            ));
            let image = PdfImage::try_new("mask".into(), &object, &ctx).unwrap();
            assert!(image.is_mask());
            assert!(matches!(image.rgba_buffer(), Err(PdfError::ColorError(_))));
            for color in [ColorRgb::new(1.0, 0.0, 0.0), ColorRgb::new(0.0, 1.0, 0.0)] {
                let rgba = image.rgba_buffer_with_stencil_color(&color).unwrap();
                assert_eq!(
                    rgba.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>(),
                    expected
                );
                assert!(
                    rgba.chunks_exact(4)
                        .all(|p| p[..3] == [color.r(), color.g(), color.b()])
                );
            }
            assert!(
                image
                    .rgba_buffer_with_stencil_color(&ColorRgb::new(f32::NAN, 0.0, 0.0))
                    .is_err()
            );
        }
        for field in [
            "/BitsPerComponent 8",
            "/ColorSpace /DeviceGray",
            "/SMask << >>",
            "/Decode [0 0.5]",
            "/Decode [0]",
            "/ImageMask /true",
        ] {
            let dict = parse(&format!("<< /Width 3 /Height 2 /ImageMask true {field} >>"));
            let object = PdfObject::PdfStream(PdfStream::new(
                dict.as_dict().unwrap().clone(),
                vec![0x40, 0xa0],
            ));
            assert!(
                PdfImage::try_new("mask".into(), &object, &ctx).is_err(),
                "{field}"
            );
        }
        let dict = parse("<< /Width 3 /Height 2 /ImageMask true >>");
        for bytes in [vec![0x40], vec![0x40, 0xa0, 0]] {
            let object =
                PdfObject::PdfStream(PdfStream::new(dict.as_dict().unwrap().clone(), bytes));
            assert!(matches!(
                PdfImage::try_new("mask".into(), &object, &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        let mut options = ParseOptions::default();
        options.limits.max_decoded_bytes = 23;
        let limited =
            PDFContext::<Cursor<Vec<u8>>>::new_without_source(PdfDict::default(), options);
        let object = PdfObject::PdfStream(PdfStream::new(
            dict.as_dict().unwrap().clone(),
            vec![0x40, 0xa0],
        ));
        assert!(matches!(
            PdfImage::try_new("mask".into(), &object, &limited),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn packed_samples_reset_at_rows_and_decode_before_quantization() {
        assert_eq!(
            unpack_image_samples(&[0xbf, 0x5f], 3, 2, 1, 1, None, 6).unwrap(),
            [255, 0, 255, 0, 255, 0]
        );
        assert_eq!(
            unpack_image_samples(&[0x1b], 4, 1, 1, 2, None, 4).unwrap(),
            [0, 85, 170, 255]
        );
        assert_eq!(
            unpack_image_samples(&[0x08, 0xff], 3, 1, 1, 4, None, 3).unwrap(),
            [0, 136, 255]
        );
        assert_eq!(
            unpack_image_samples(&[0, 0, 0x80, 0, 0xff, 0xff], 3, 1, 1, 16, None, 3).unwrap(),
            [0, 128, 255]
        );
        let mut endpoints = [0.0; 8];
        endpoints[0] = 1.0;
        assert_eq!(
            unpack_image_samples(&[0x80, 0], 1, 1, 1, 16, Some(endpoints), 1).unwrap(),
            [127]
        );
        endpoints[0] = 0.0;
        endpoints[1] = 0.5;
        // Original 16-bit precision gives 64, whereas quantizing to 8 bits first gives 65.
        assert_eq!(
            unpack_image_samples(&[0x81, 0], 1, 1, 1, 16, Some(endpoints), 1).unwrap(),
            [64]
        );
        assert!(matches!(
            unpack_image_samples(&[0], 1, 1, 1, 16, None, 1),
            Err(PdfError::ObjectError(_))
        ));
        assert!(matches!(
            unpack_image_samples(&[0], 4, 1, 1, 2, None, 3),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn decode_arrays_support_default_inversion_and_clamped_ranges() {
        use crate::{
            objects::{pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber},
            parser::options::ParseOptions,
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (values, expected) in [
            (vec![0.0, 1.0], vec![0, 128, 255]),
            (vec![1.0, 0.0], vec![255, 127, 0]),
            (vec![-1.0, 2.0], vec![0, 129, 255]),
            (vec![0.5, 0.5], vec![128, 128, 128]),
        ] {
            let mut array = PdfArray::default();
            for value in values {
                array
                    .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(value)))
                    .unwrap();
            }
            let mut bytes = vec![0, 128, 255];
            apply_image_decode(&mut bytes, Some(1), Some(&PdfObject::PdfArray(array)), &ctx)
                .unwrap();
            assert_eq!(bytes, expected);
        }
        let bad = PdfObject::PdfArray(PdfArray::default());
        assert!(apply_image_decode(&mut [128], Some(1), Some(&bad), &ctx).is_err());
        assert!(
            apply_image_decode(
                &mut [128],
                Some(1),
                Some(&PdfObject::PdfNumber(
                    crate::objects::pdf_number::PdfNumber::new(1.0)
                )),
                &ctx
            )
            .is_err()
        );
    }

    #[test]
    fn soft_mask_parser_rejects_invalid_fields_lengths_and_nested_masks() {
        use crate::{
            objects::{
                pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream,
            },
            parser::options::ParseOptions,
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let mut mask = PdfDict::default();
        for (key, value) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
            mask.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
                .unwrap();
        }
        for (key, value) in [("Width", 1), ("Height", 1), ("BitsPerComponent", 8)] {
            mask.try_insert(
                key.into(),
                PdfObject::PdfNumber(PdfNumber::new(value as f32)),
            )
            .unwrap();
        }
        let mut base = mask.clone();
        base.try_insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        )
        .unwrap();
        let image_with_mask = |dict: PdfDict, alpha: Vec<u8>| {
            let mut image = base.clone();
            image
                .try_insert(
                    "SMask".into(),
                    PdfObject::PdfStream(PdfStream::new(dict, alpha)),
                )
                .unwrap();
            PdfObject::PdfStream(PdfStream::new(image, vec![10, 20, 30]))
        };
        let valid = image_with_mask(mask.clone(), vec![128]);
        for (bits, bytes, expected) in [
            (1, vec![0xff], 255),
            (2, vec![0x80], 170),
            (4, vec![0x80], 136),
            (16, vec![0x80, 0], 128),
        ] {
            let mut packed = mask.clone();
            packed
                .try_insert(
                    "BitsPerComponent".into(),
                    PdfObject::PdfNumber(PdfNumber::new(bits as f32)),
                )
                .unwrap();
            assert_eq!(
                PdfImage::try_new("test".into(), &image_with_mask(packed, bytes), &ctx)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                [10, 20, 30, expected]
            );
        }
        assert_eq!(
            PdfImage::try_new("test".into(), &valid, &ctx)
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            vec![10, 20, 30, 128]
        );
        let mut inverted = mask.clone();
        let mut decode = crate::objects::pdf_array::PdfArray::default();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(0.0)))
            .unwrap();
        inverted
            .try_insert("Decode".into(), PdfObject::PdfArray(decode))
            .unwrap();
        inverted
            .try_insert(
                "ImageMask".into(),
                PdfObject::PdfBool(crate::objects::pdf_bool::PdfBool::new(b"false")),
            )
            .unwrap();
        assert_eq!(
            PdfImage::try_new("test".into(), &image_with_mask(inverted, vec![128]), &ctx)
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            vec![10, 20, 30, 127]
        );
        for key in [
            "Width",
            "Height",
            "BitsPerComponent",
            "Subtype",
            "ColorSpace",
        ] {
            let mut missing = mask.clone();
            missing.remove(key);
            assert!(matches!(
                PdfImage::try_new("test".into(), &image_with_mask(missing, vec![128]), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        for (key, value) in [
            ("Width", PdfObject::PdfNumber(PdfNumber::new(2.0))),
            ("Height", PdfObject::PdfNumber(PdfNumber::new(0.5))),
            (
                "BitsPerComponent",
                PdfObject::PdfNumber(PdfNumber::new(16.0)),
            ),
            (
                "ColorSpace",
                PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
            ),
            ("Subtype", PdfObject::PdfName(PdfName::new("Wrong".into()))),
            ("SMask", valid.clone()),
            ("Matte", PdfObject::PdfNumber(PdfNumber::new(1.0))),
            ("Decode", PdfObject::PdfNumber(PdfNumber::new(1.0))),
        ] {
            let mut invalid = mask.clone();
            invalid.try_insert(key.into(), value).unwrap();
            assert!(
                PdfImage::try_new("test".into(), &image_with_mask(invalid, vec![128]), &ctx)
                    .is_err(),
                "{key}"
            );
        }
        for alpha in [vec![], vec![0, 255]] {
            assert!(matches!(
                PdfImage::try_new("test".into(), &image_with_mask(mask.clone(), alpha), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
    }

    #[test]
    fn rgba_conversion_checks_data_length_and_allocation_budget() {
        let mut image = PdfImage {
            name: "test".into(),
            width: 1,
            height: 1,
            colorspace: ColorSpace::DeviceRgb(DeviceRgb::new()),
            data: vec![10, 20, 30],
            is_mask: false,
            is_inline: false,
            pixel_count: 1,
            max_rgba_bytes: 4,
            soft_mask: None,
            color_sample_decode: None,
        };
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 255]);
        image.soft_mask = Some(vec![128]);
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 128]);
        image.soft_mask = Some(vec![]);
        assert!(image.rgba_buffer().is_err());
        image.soft_mask = None;
        image.max_rgba_bytes = 3;
        assert!(matches!(
            image.rgba_buffer(),
            Err(PdfError::ResourceLimit(_))
        ));
        image.max_rgba_bytes = 4;
        image.data.pop();
        assert!(image.rgba_buffer().is_err());
        image.data.extend_from_slice(&[30, 40]);
        assert!(image.rgba_buffer().is_err());
    }
}

impl PdfImage {
    pub(crate) fn mark_inline(mut self) -> Self { self.is_inline = true; self }
    pub fn try_new<R: Seek + Read>(
        name: String,
        image: &PdfObject,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        let resolved = ctx.resolve(image)?;
        let image = &resolved;
        let stream = image
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("image must be a stream".into()))?;
        let filter=stream.dict().get("Filter").map(|f|crate::objects::pdf_stream::resolve_filter_setting(f,ctx,false)).transpose()?;
        let is_jpx=match filter.as_ref() {
            Some(PdfObject::PdfName(name))=>name.name()=="JPXDecode",
            Some(PdfObject::PdfArray(array)) if array.len()==1=>array.get(0).and_then(PdfObject::as_name).is_some_and(|n|n.name()=="JPXDecode"),
            Some(PdfObject::PdfArray(array)) if array.into_iter().any(|f|f.as_name().is_some_and(|n|n.name()=="JPXDecode"))=>return Err(PdfError::UnsupportedFeature("JPX image filter chain outside single-stage baseline".into())),
            _=>false
        };
        let is_jbig2=filter.as_ref().is_some_and(|filter|match filter {
            PdfObject::PdfName(name)=>name.name()=="JBIG2Decode",
            PdfObject::PdfArray(array)=>array.into_iter().any(|f|f.as_name().is_some_and(|n|n.name()=="JBIG2Decode")),
            _=>false
        });
        let jpx = if is_jpx {
            Some(crate::filter::jpx::inspect(stream.raw_data(), crate::filter::FilterContext { max_decoded_bytes:ctx.limits().max_decoded_bytes, max_image_pixels:ctx.limits().max_image_pixels, ..Default::default() })?)
        } else { None };
        if let Some(info) = jpx {
            let number=|key|image.get_attr(key).map(|v|ctx.resolve_owned(v)).transpose().map(|v|v.and_then(|v|v.as_u32()));
            if image.get_attr("SMaskInData").is_some() && number("SMaskInData")?!=Some(0) {return Err(PdfError::UnsupportedFeature("JPX image SMaskInData/alpha".into()));}
            if number("Width")?!=Some(info.width) || number("Height")?!=Some(info.height) {return Err(PdfError::ObjectError("JPX dimensions disagree with image dictionary".into()));}
        }
        if let Some(subtype) = image.get_attr("Subtype") {
            let subtype = ctx.resolve(subtype)?;
            if subtype.as_name().map(|name| name.bytes()) != Some(b"Image".as_slice()) {
                return Err(PdfError::ObjectError("image Subtype must be /Image".into()));
            }
        }
        let is_mask = if let Some(mask) = image.get_attr("ImageMask") {
            let mask = ctx.resolve(mask)?;
            let is_mask = mask
                .as_bool()
                .ok_or_else(|| PdfError::ObjectError("image ImageMask must be boolean".into()))?
                .value();
            is_mask
        } else {
            false
        };
        if is_mask && jpx.is_some() {return Err(PdfError::UnsupportedFeature("JPX stencil image outside baseline".into()));}
        if is_jbig2 && (!is_mask || image.get_attr("BitsPerComponent").is_some()) {
            let bits=image.get_attr("BitsPerComponent").map(|v|ctx.resolve_owned(v)).transpose()?;
            if bits.as_ref().and_then(PdfObject::as_u32)!=Some(1) {return Err(PdfError::ObjectError("JBIG2 images require 1-bit samples".into()));}
        }
        if is_mask {
            for key in ["ColorSpace", "SMask"] {
                if let Some(value) = image.get_attr(key) {
                    if !matches!(ctx.resolve(value)?, PdfObject::PdfNull) {
                        return Err(PdfError::ObjectError(format!(
                            "stencil image must not specify {key}"
                        )));
                    }
                }
            }
        }
        for key in ["Mask"] {
            if let Some(value) = image.get_attr(key) {
                let value = ctx.resolve(value)?;
                if !matches!(value, PdfObject::PdfNull) {
                    return Err(PdfError::UnsupportedFeature(format!("image {key}")));
                }
            }
        }
        let dimension = |key: &str| -> PdfResult<u32> {
            let object = ctx.resolve(
                image
                    .get_attr(key)
                    .ok_or_else(|| PdfError::ObjectError(format!("image {key} is missing")))?,
            )?;
            let value = object
                .as_number()
                .ok_or_else(|| PdfError::ObjectError(format!("image {key} must be an integer")))?
                .as_u32_checked()?;
            if value == 0 {
                return Err(PdfError::ObjectError(format!(
                    "image {key} must be positive"
                )));
            }
            Ok(value)
        };
        let height = dimension("Height")?;
        let width = dimension("Width")?;
        if u64::from(width) * u64::from(height) > ctx.limits().max_image_pixels {
            return Err(PdfError::ResourceLimit("image pixel count".into()));
        }
        let pixel_count = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| PdfError::ResourceLimit("image pixel count overflow".into()))?;
        pixel_count
            .checked_mul(4)
            .filter(|bytes| *bytes <= ctx.limits().max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let bits = if jpx.is_some() { 8 } else if let Some(bits) = image.get_attr("BitsPerComponent") {
            let bits = ctx.resolve(bits)?;
            let bits = bits
                .as_u32()
                .filter(|bits| matches!(bits, 1 | 2 | 4 | 8 | 16))
                .ok_or_else(|| {
                    PdfError::ObjectError(
                        "image BitsPerComponent must be an integer 1, 2, 4, 8 or 16".into(),
                    )
                })?;
            bits as usize
        } else {
            if is_mask { 1 } else { 8 }
        };
        if is_mask && bits != 1 {
            return Err(PdfError::ObjectError(
                "stencil image BitsPerComponent must be 1".into(),
            ));
        }
        let stencil_decode = if is_mask {
            let endpoints = image_decode_endpoints(Some(1), image.get_attr("Decode"), ctx)?;
            if endpoints.is_some_and(|e| !matches!((e[0], e[1]), (0.0, 1.0) | (1.0, 0.0))) {
                return Err(PdfError::ObjectError(
                    "stencil image Decode must be [0 1] or [1 0]".into(),
                ));
            }
            endpoints
        } else {
            None
        };
        let colorspace = if is_mask {
            // Internal sample carrier only: a stencil has no PDF ColorSpace.
            ColorSpace::DeviceGray(crate::color::device_gray::DeviceGray::new())
        } else {
            if let Some(colorspace_obj) = image.get_attr("ColorSpace") {
                let color = ColorSpace::try_new(colorspace_obj, ctx)?;
                if let Some(info) = jpx {
                    if !matches!((&color, info.color_space),(ColorSpace::DeviceGray(_),crate::filter::jpx::JpxColorSpace::Gray)|(ColorSpace::DeviceRgb(_),crate::filter::jpx::JpxColorSpace::Rgb)) { return Err(PdfError::UnsupportedFeature("JPX dictionary colour override outside baseline".into())); }
                }
                color
            } else if let Some(info) = jpx {
                match info.color_space {
                    crate::filter::jpx::JpxColorSpace::Gray => ColorSpace::DeviceGray(crate::color::device_gray::DeviceGray::new()),
                    crate::filter::jpx::JpxColorSpace::Rgb => ColorSpace::DeviceRgb(crate::color::device_rgb::DeviceRgb::new()),
                }
            } else {return Err(PdfError::ColorError("image ColorSpace is missing".into()));}
        };
        let indexed = matches!(colorspace, ColorSpace::Indexed(_));
        if is_jbig2 && !matches!(colorspace,ColorSpace::DeviceGray(_)) {return Err(PdfError::UnsupportedFeature("JBIG2 colour space outside Gray/stencil baseline".into()));}
        let indexed_decode = if indexed {
            image_decode_endpoints(Some(1), image.get_attr("Decode"), ctx)?
        } else {
            None
        };
        let color_sample_decode = match &colorspace {
            ColorSpace::CalGray(_)
            | ColorSpace::CalRgb(_)
            | ColorSpace::Lab(_)
            | ColorSpace::IccBased(_)
            | ColorSpace::Separation(_)
            | ColorSpace::DeviceN(_) => {
                let count = colorspace.number_of_components()?;
                let slots = count
                    .checked_mul(2)
                    .filter(|n| {
                        n.checked_mul(std::mem::size_of::<f64>())
                            .is_some_and(|bytes| bytes <= ctx.limits().max_decoded_bytes)
                    })
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("image Decode endpoint budget".into())
                    })?;
                let mut defaults = Vec::new();
                defaults.try_reserve_exact(slots).map_err(|_| {
                    PdfError::ResourceLimit("image Decode endpoint allocation".into())
                })?;
                defaults.resize(slots, 0.0);
                for pair in defaults.chunks_exact_mut(2) {
                    pair[1] = 1.0;
                }
                match &colorspace {
                    ColorSpace::Lab(space) => {
                        for (target, value) in defaults.iter_mut().zip(space.component_ranges()) {
                            *target = f64::from(value);
                        }
                    }
                    ColorSpace::IccBased(space) => {
                        for (target, value) in defaults.iter_mut().zip(space.range()) {
                            *target = f64::from(*value);
                        }
                    }
                    _ => {}
                }
                if let Some(decode) = image.get_attr("Decode") {
                    let decode = ctx.resolve(decode)?;
                    if !matches!(decode, PdfObject::PdfNull) {
                        let array = decode
                            .as_array()
                            .filter(|array| array.len() == slots)
                            .ok_or_else(|| {
                                PdfError::ObjectError(
                                    "image Decode component count mismatch".into(),
                                )
                            })?;
                        for (target, value) in defaults.iter_mut().zip(array) {
                            *target = f64::from(
                                ctx.resolve(value)?
                                    .as_f32()
                                    .filter(|value| value.is_finite())
                                    .ok_or_else(|| {
                                        PdfError::ObjectError(
                                            "image Decode requires finite numbers".into(),
                                        )
                                    })?,
                            );
                        }
                    }
                }
                Some((bits, defaults))
            }
            _ => None,
        };
        let mut data = stream.decode_data(ctx)?;
        let components = match &colorspace {
            ColorSpace::DeviceGray(_) => Some(1),
            ColorSpace::DeviceRgb(_) => Some(3),
            ColorSpace::DeviceCmyk(_) => Some(4),
            _ => None,
        };
        if color_sample_decode.is_some() {
            let count = colorspace.number_of_components()?;
            let row_bytes = (width as usize)
                .checked_mul(count)
                .and_then(|n| n.checked_mul(bits))
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            if row_bytes.checked_mul(height as usize) != Some(data.len()) {
                return Err(PdfError::ObjectError(
                    "packed image sample length mismatch".into(),
                ));
            }
        } else if indexed {
            data = unpack_image_samples_inner(
                &data,
                width as usize,
                height as usize,
                1,
                bits,
                indexed_decode,
                ctx.limits().max_decoded_bytes,
                true,
            )?;
        } else if bits == 8 {
            // Decode is ignored for non-stencil JPX images (ISO image dictionary).
            if jpx.is_none() { apply_image_decode(&mut data, components, image.get_attr("Decode"), ctx)?; }
        } else {
            let components = components.ok_or_else(|| {
                PdfError::UnsupportedFeature("packed image samples for this color space".into())
            })?;
            let endpoints = if is_mask {
                stencil_decode
            } else {
                image_decode_endpoints(Some(components), image.get_attr("Decode"), ctx)?
            };
            data = unpack_image_samples(
                &data,
                width as usize,
                height as usize,
                components,
                bits,
                endpoints,
                ctx.limits().max_decoded_bytes,
            )?;
        }
        let soft_mask = match image.get_attr("SMask") {
            None | Some(PdfObject::PdfNull) => None,
            Some(value) => {
                let mask = ctx.resolve(value)?;
                if matches!(mask, PdfObject::PdfNull) {
                    None
                } else {
                    let mask = mask.as_stream().ok_or_else(|| {
                        PdfError::ObjectError("SMask must be an image stream".into())
                    })?;
                    for (key, expected) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
                        let value = mask.dict().get(key).ok_or_else(|| {
                            PdfError::ObjectError("SMask image field missing".into())
                        })?;
                        let value = ctx.resolve(value)?;
                        if value.as_name().map(|name| name.name()) != Some(expected) {
                            return Err(PdfError::UnsupportedFeature(
                                "SMask requires Image/DeviceGray".into(),
                            ));
                        }
                    }
                    for (key, expected) in [("Width", width), ("Height", height)] {
                        let value = mask.dict().get(key).ok_or_else(|| {
                            PdfError::ObjectError("SMask dimensions/components missing".into())
                        })?;
                        let value = ctx.resolve(value)?;
                        let actual = value
                            .as_number()
                            .ok_or_else(|| {
                                PdfError::ObjectError(
                                    "SMask dimensions/components must be integers".into(),
                                )
                            })?
                            .as_u32_checked()?;
                        if actual != expected {
                            return Err(PdfError::UnsupportedFeature(
                                "SMask requires matching dimensions".into(),
                            ));
                        }
                    }
                    let mask_bits =
                        ctx.resolve(mask.dict().get("BitsPerComponent").ok_or_else(|| {
                            PdfError::ObjectError("SMask BitsPerComponent missing".into())
                        })?)?;
                    let mask_bits = mask_bits
                        .as_u32()
                        .filter(|bits| matches!(bits, 1 | 2 | 4 | 8 | 16))
                        .ok_or_else(|| {
                            PdfError::ObjectError("invalid SMask BitsPerComponent".into())
                        })? as usize;
                    for key in ["Mask", "SMask", "Matte"] {
                        if let Some(value) = mask.dict().get(key) {
                            if !matches!(ctx.resolve(value)?, PdfObject::PdfNull) {
                                return Err(PdfError::UnsupportedFeature(format!("SMask {key}")));
                            }
                        }
                    }
                    if let Some(value) = mask.dict().get("ImageMask") {
                        let value = ctx.resolve(value)?;
                        let flag = value
                            .as_bool()
                            .ok_or_else(|| {
                                PdfError::ObjectError("SMask ImageMask must be boolean".into())
                            })?
                            .value();
                        if flag {
                            return Err(PdfError::UnsupportedFeature("SMask stencil mask".into()));
                        }
                    }
                    let mut alpha = mask.decode_data(ctx)?;
                    if mask_bits == 8 {
                        apply_image_decode(&mut alpha, Some(1), mask.dict().get("Decode"), ctx)?;
                    } else {
                        let endpoints =
                            image_decode_endpoints(Some(1), mask.dict().get("Decode"), ctx)?;
                        alpha = unpack_image_samples(
                            &alpha,
                            width as usize,
                            height as usize,
                            1,
                            mask_bits,
                            endpoints,
                            ctx.limits().max_decoded_bytes,
                        )?;
                    }
                    if alpha.len() != pixel_count {
                        return Err(PdfError::ObjectError(
                            "SMask data length does not match dimensions".into(),
                        ));
                    }
                    Some(alpha)
                }
            }
        };

        Ok(Self {
            name,
            height,
            width,
            colorspace,
            data,
            is_inline: false,
            is_mask,
            pixel_count,
            max_rgba_bytes: ctx.limits().max_decoded_bytes,
            soft_mask,
            color_sample_decode,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn width(&self) -> f32 {
        self.width as f32
    }
    pub fn height(&self) -> f32 {
        self.height as f32
    }
    pub fn pixel_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub(crate) fn retained_sample_bytes(&self) -> PdfResult<usize> {
        self.data
            .len()
            .checked_add(self.soft_mask.as_ref().map_or(0, Vec::len))
            .and_then(|n| {
                self.color_sample_decode
                    .as_ref()
                    .map_or(Some(n), |(_, endpoints)| {
                        endpoints
                            .capacity()
                            .checked_mul(std::mem::size_of::<f64>())
                            .and_then(|bytes| n.checked_add(bytes))
                    })
            })
            .ok_or_else(|| PdfError::ResourceLimit("image retained samples".into()))
    }

    pub fn colorspace(&self) -> &ColorSpace {
        &self.colorspace
    }
    pub fn is_mask(&self) -> bool {
        self.is_mask
    }

    pub fn is_inline(&self) -> bool {
        self.is_inline
    }
    pub fn rgba_buffer(&self) -> PdfResult<Vec<u8>> {
        self.rgba_buffer_inner(None)
    }

    /// Apply the current nonstroking color to a stencil's transparent coverage.
    pub fn rgba_buffer_with_stencil_color(
        &self,
        color: &crate::color::value::ColorRgb,
    ) -> PdfResult<Vec<u8>> {
        if !self.is_mask
            || [color.r, color.g, color.b]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(PdfError::ColorError(
                "stencil color requires a stencil and finite RGB components in [0,1]".into(),
            ));
        }
        self.rgba_buffer_inner(Some(color))
    }

    fn rgba_buffer_inner(
        &self,
        stencil_color: Option<&crate::color::value::ColorRgb>,
    ) -> PdfResult<Vec<u8>> {
        if self.is_mask && stencil_color.is_none() {
            return Err(PdfError::ColorError(
                "stencil RGBA conversion requires the current nonstroking color".into(),
            ));
        }
        let components = match &self.colorspace {
            ColorSpace::DeviceGray(_) => 1,
            ColorSpace::DeviceRgb(_) => 3,
            ColorSpace::DeviceCmyk(_) => 4,
            ColorSpace::Indexed(_) => 1,
            ColorSpace::CalGray(_)
            | ColorSpace::CalRgb(_)
            | ColorSpace::Lab(_)
            | ColorSpace::IccBased(_)
            | ColorSpace::Separation(_)
            | ColorSpace::DeviceN(_) => self.colorspace.number_of_components()?,
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "image RGBA conversion for this color space".into(),
                ));
            }
        };
        let input_size = self
            .pixel_count
            .checked_mul(components)
            .ok_or_else(|| PdfError::ResourceLimit("image data size overflow".into()))?;
        if self.color_sample_decode.is_none() && self.data.len() != input_size {
            return Err(PdfError::ObjectError(
                "image data length does not match dimensions/components".into(),
            ));
        }
        let output_size = self
            .pixel_count
            .checked_mul(4)
            .filter(|n| *n <= self.max_rgba_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(output_size)
            .map_err(|_| PdfError::ResourceLimit("unable to allocate image RGBA buffer".into()))?;
        if let Some((bits, endpoints)) = &self.color_sample_decode {
            let bits = *bits;
            let row_samples = (self.width as usize)
                .checked_mul(components)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            let row_bytes = row_samples
                .checked_mul(bits)
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            if row_bytes.checked_mul(self.height as usize) != Some(self.data.len()) {
                return Err(PdfError::ObjectError(
                    "packed image sample length mismatch".into(),
                ));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(components)
                .map_err(|_| PdfError::ResourceLimit("image color component allocation".into()))?;
            values.resize(components, 0.0);
            let mut color = crate::color::value::ColorValue::new(values);
            let maximum = ((1u32 << bits) - 1) as f64;
            for row in self.data.chunks_exact(row_bytes) {
                for pixel in 0..self.width as usize {
                    for component in 0..components {
                        let offset = (pixel * components + component) * bits;
                        let sample = if bits == 16 {
                            u16::from_be_bytes([row[offset / 8], row[offset / 8 + 1]]) as u32
                        } else {
                            (u32::from(row[offset / 8]) >> (8 - bits - offset % 8))
                                & ((1 << bits) - 1)
                        };
                        color.values_mut()[component] = (endpoints[component * 2]
                            + sample as f64 / maximum
                                * (endpoints[component * 2 + 1] - endpoints[component * 2]))
                            as f32;
                    }
                    let rgb = self.colorspace.rgb(&color)?;
                    rgba.extend_from_slice(&[rgb.r(), rgb.g(), rgb.b(), 255]);
                }
            }
        } else {
            match &self.colorspace {
                ColorSpace::DeviceGray(g) => {
                    for &g in self.data.iter() {
                        if let Some(color) = stencil_color {
                            rgba.extend_from_slice(&[color.r(), color.g(), color.b(), 255 - g]);
                        } else {
                            rgba.extend_from_slice(&[g, g, g, 255]);
                        }
                    }
                }
                ColorSpace::DeviceRgb(_) => {
                    for chunk in self.data.chunks_exact(3) {
                        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                    }
                }
                ColorSpace::DeviceCmyk(_) => {
                    for chunk in self.data.chunks_exact(4) {
                        let (c, m, y, k) = (
                            chunk[0] as f32 / 255.0,
                            chunk[1] as f32 / 255.0,
                            chunk[2] as f32 / 255.0,
                            chunk[3] as f32 / 255.0,
                        );
                        let r = (255.0 * (1.0 - c) * (1.0 - k)) as u8;
                        let g = (255.0 * (1.0 - m) * (1.0 - k)) as u8;
                        let b = (255.0 * (1.0 - y) * (1.0 - k)) as u8;
                        rgba.extend_from_slice(&[r, g, b, 255]);
                    }
                }
                ColorSpace::Indexed(space) => {
                    // Cache only entries actually encountered: eagerly converting the
                    // entire table could fail on an unused tint-transform input.
                    // Fixed stack storage bounds the cache independently of image size.
                    let mut palette: [Option<[u8; 4]>; 256] = [None; 256];
                    for &index in &self.data {
                        let pixel = match palette[index as usize] {
                            Some(pixel) => pixel,
                            None => {
                                let rgb =
                                    space.rgb(&crate::color::value::ColorValue::new(vec![
                                        index as f32,
                                    ]))?;
                                let pixel = [rgb.r(), rgb.g(), rgb.b(), 255];
                                palette[index as usize] = Some(pixel);
                                pixel
                            }
                        };
                        rgba.extend_from_slice(&pixel);
                    }
                }
                _ => {}
            }
        }
        if let Some(alpha) = &self.soft_mask {
            if alpha.len() != self.pixel_count {
                return Err(PdfError::ObjectError("SMask alpha length mismatch".into()));
            }
            for (pixel, &alpha) in rgba.chunks_exact_mut(4).zip(alpha) {
                pixel[3] = alpha;
            }
        }
        Ok(rgba)
    }
}

fn apply_image_decode<R: Read + Seek>(
    data: &mut [u8],
    components: Option<usize>,
    decode: Option<&PdfObject>,
    ctx: &PDFContext<R>,
) -> PdfResult<()> {
    let Some(endpoints) = image_decode_endpoints(components, decode, ctx)? else {
        return Ok(());
    };
    let components = components.unwrap();
    for (index, sample) in data.iter_mut().enumerate() {
        let component = index % components;
        let low = endpoints[component * 2];
        let high = endpoints[component * 2 + 1];
        *sample = ((low + f64::from(*sample) / 255.0 * (high - low)).clamp(0.0, 1.0) * 255.0)
            .round() as u8;
    }
    Ok(())
}

fn image_decode_endpoints<R: Read + Seek>(
    components: Option<usize>,
    decode: Option<&PdfObject>,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<[f64; 8]>> {
    let Some(decode) = decode else {
        return Ok(None);
    };
    let decode = ctx.resolve(decode)?;
    if matches!(decode, PdfObject::PdfNull) {
        return Ok(None);
    }
    let components = components
        .ok_or_else(|| PdfError::UnsupportedFeature("image Decode for this color space".into()))?;
    if components > 4 {
        return Err(PdfError::UnsupportedFeature(
            "fixed image Decode supports at most four components".into(),
        ));
    }
    let array = decode
        .as_array()
        .ok_or_else(|| PdfError::ObjectError("image Decode must be an array".into()))?;
    if array.len() != components * 2 {
        return Err(PdfError::ObjectError(
            "image Decode component count mismatch".into(),
        ));
    }
    let mut endpoints = [0.0f64; 8];
    for (index, value) in array.into_iter().enumerate() {
        let value = ctx.resolve(value)?;
        endpoints[index] = f64::from(
            value
                .as_f32()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    PdfError::ObjectError("image Decode requires finite numbers".into())
                })?,
        );
    }
    Ok(Some(endpoints))
}

fn unpack_image_samples(
    source: &[u8],
    width: usize,
    height: usize,
    components: usize,
    bits: usize,
    endpoints: Option<[f64; 8]>,
    limit: usize,
) -> PdfResult<Vec<u8>> {
    unpack_image_samples_inner(
        source, width, height, components, bits, endpoints, limit, false,
    )
}

fn unpack_image_samples_inner(
    source: &[u8],
    width: usize,
    height: usize,
    components: usize,
    bits: usize,
    endpoints: Option<[f64; 8]>,
    limit: usize,
    indexed: bool,
) -> PdfResult<Vec<u8>> {
    let overflow = || PdfError::ResourceLimit("image sample size overflow".into());
    let row_samples = width.checked_mul(components).ok_or_else(overflow)?;
    let row_bytes = row_samples
        .checked_mul(bits)
        .and_then(|n| n.checked_add(7))
        .ok_or_else(overflow)?
        / 8;
    if source.len() != row_bytes.checked_mul(height).ok_or_else(overflow)? {
        return Err(PdfError::ObjectError(
            "packed image sample length mismatch".into(),
        ));
    }
    let size = row_samples
        .checked_mul(height)
        .filter(|n| *n <= limit)
        .ok_or_else(overflow)?;
    let mut output = Vec::new();
    output.try_reserve_exact(size).map_err(|_| overflow())?;
    let maximum = ((1u32 << bits) - 1) as f64;
    for row in source.chunks_exact(row_bytes) {
        for index in 0..row_samples {
            let offset = index * bits;
            let sample = if bits == 16 {
                u16::from_be_bytes([row[offset / 8], row[offset / 8 + 1]]) as u32
            } else {
                (u32::from(row[offset / 8]) >> (8 - bits - offset % 8)) & ((1 << bits) - 1)
            };
            let (low, high) = endpoints.map_or((0.0, if indexed { maximum } else { 1.0 }), |e| {
                let component = index % components;
                (e[component * 2], e[component * 2 + 1])
            });
            let decoded = low + sample as f64 / maximum * (high - low);
            output.push(if indexed {
                decoded.clamp(0.0, 255.0).round() as u8
            } else {
                (decoded.clamp(0.0, 1.0) * 255.0).round() as u8
            });
        }
    }
    Ok(output)
}

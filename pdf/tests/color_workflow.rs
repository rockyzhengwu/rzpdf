use pdf::{
    device::Device,
    document::{
        PdfDocument,
        save_options::{SaveOptions, StreamCompression},
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    page::{
        image_object::ImageObject, page_builder::PageBuilder, path_object::PathObject,
        text_object::TextObject,
    },
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[derive(Default)]
struct ColorDevice {
    colors: Vec<[u8; 3]>,
    text_modes: Vec<pdf::page::text_state::TextRenderingMode>,
    images: usize,
    indexed_pixels: Vec<Vec<u8>>,
    shadings: Vec<pdf::page::shading_object::ShadingObject>,
}

#[test]
fn shading_resource_alias_state_and_replay_survive_strict_save() {
    use pdf::{document::resource_manager::ResourceKind, page::page_object::PageObject};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let gray = doc
        .resources()
        .register_color_space(0, parse_object("/DeviceGray"))
        .unwrap();
    let none = doc
        .resources()
        .register_color_space(
            0,
            parse_object(
                "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            ),
        )
        .unwrap();
    let object = parse_object(&format!(
        "<< /ShadingType 2 /ColorSpace /{gray} /Coords [0 0 10 0] /Extend [true true] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>"
    ));
    let id = doc.try_insert_object(object).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Shading, id)
        .unwrap();
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap(),
        name
    );
    doc.replace_page_contents(
        0,
        format!("/{none} cs q 2 0 0 3 5 7 cm /{name} sh Q").into_bytes(),
    )
    .unwrap();
    let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
        let mut page = doc.get_page(0).unwrap();
        let list = page.display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        assert_eq!(list.objects().len(), 1);
        let PageObject::Shading(object) = &list.objects()[0] else {
            panic!("expected shading")
        };
        assert!(object.source().is_some());
        assert_eq!(object.resource_name, name);
        assert_eq!(object.shading.color_at(0.5).unwrap().r(), 128);
        assert!(
            object
                .graphic_state
                .color_state
                .fill_color_space
                .is_nonpainting()
        );
        let mut direct = ColorDevice::default();
        page.display(&mut direct).unwrap();
        let mut replay = ColorDevice::default();
        list.replay(&mut replay).unwrap();
        assert_eq!(direct.shadings.len(), 1);
        assert_eq!(replay.shadings.len(), 1);
        let components = |matrix: &pdf::geom::matrix::Matrix| {
            [
                matrix.a(),
                matrix.b(),
                matrix.c(),
                matrix.d(),
                matrix.e(),
                matrix.f(),
            ]
        };
        assert_eq!(
            components(&direct.shadings[0].matrix),
            components(&replay.shadings[0].matrix)
        );
    };
    verify(&doc);
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
    }
}

#[test]
fn type4_conditional_colors_and_images_survive_strict_save() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    for devicen in [false, true] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let domain = if devicen { "0 1 0 1" } else { "0 1" };
        let dict = parse_object(&format!(
            "<< /FunctionType 4 /Domain [{domain}] /Range [0 1 0 1 0 1] >>"
        ))
        .into_dict()
        .unwrap();
        let program = if devicen {
            b"{add .5 gt {1 0 0} {0 1 0} ifelse}".as_slice()
        } else {
            b"{.5 gt {1 0 0} {0 1 0} ifelse}"
        };
        let function = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, program.to_vec())))
            .unwrap();
        let family = if devicen {
            "/DeviceN [/A /B]"
        } else {
            "/Separation /Spot"
        };
        let space = format!(
            "[{family} /DeviceRGB {} {} R]",
            function.obj_num(),
            function.gen_num()
        );
        let color = doc
            .resources()
            .register_color_space(0, parse_object(&space))
            .unwrap();
        let samples = if devicen {
            vec![32, 32, 128, 128]
        } else {
            vec![64, 191]
        };
        let dict = parse_object(&format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace {space} >>")).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, samples)))
            .unwrap();
        let image = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        let low = if devicen { "0.125 0.125" } else { "0.25" };
        let high = if devicen { "0.5 0.5" } else { "0.75" };
        doc.replace_page_contents(
            0,
            format!("/{color} cs {low} scn 0 0 10 10 re f {high} scn 10 0 10 10 re f /{image} Do")
                .into_bytes(),
        )
        .unwrap();
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let mut device = ColorDevice::default();
            doc.get_page(0).unwrap().display(&mut device).unwrap();
            assert_eq!(device.colors, vec![[0, 255, 0], [255, 0, 0]]);
            assert_eq!(
                device.indexed_pixels,
                vec![vec![0, 255, 0, 255, 255, 0, 0, 255]]
            );
        };
        verify(&doc);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        }
    }
}

#[test]
fn six_component_devicen_images_preserve_rows_decode_and_save() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    for reverse in [false, true] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let function = parse_object("<< /FunctionType 0 /Domain [0 1 0 1 0 1 0 1 0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 1 1 1 1 2] /BitsPerSample 8 >>").into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                function,
                vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 255, 255, 0],
            )))
            .unwrap();
        let decode = if reverse {
            "/Decode [0 1 0 1 0 1 0 1 0 1 1 0]"
        } else {
            ""
        };
        let dict = parse_object(&format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 1 /ColorSpace [/DeviceN [/A /B /None /C /D /E] /DeviceRGB {} {} R] {decode} >>", id.obj_num(), id.gen_num())).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                dict,
                vec![0x02, 0x00, 0x06, 0x10],
            )))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let mut expected = vec![
            0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 255, 255, 0, 255,
        ];
        if reverse {
            expected.rotate_left(8);
        }
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let mut device = ColorDevice::default();
            doc.get_page(0).unwrap().display(&mut device).unwrap();
            assert_eq!(device.images, 1);
            assert_eq!(device.indexed_pixels, vec![expected.clone()]);
        };
        verify(&doc);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        }
    }
}

#[test]
fn stencil_images_follow_current_fill_and_preserve_none_objects_on_save() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let none = doc
        .resources()
        .register_color_space(
            0,
            parse_object(
                "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            ),
        )
        .unwrap();
    let dict =
        parse_object("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ImageMask true >>")
            .into_dict()
            .unwrap();
    let object = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![0x40])))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, object)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/{none} cs /{name} Do 1 0 0 rg /{name} Do").into_bytes(),
    )
    .unwrap();
    let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 2);
        let mut device = ColorDevice::default();
        list.replay(&mut device).unwrap();
        assert_eq!(device.images, 1);
    };
    verify(&doc);
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
    }
}

#[test]
fn indexed_image_palette_and_decode_survive_strict_save() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse_object("<< /Type /XObject /Subtype /Image /Width 3 /Height 2 /BitsPerComponent 2 /ColorSpace [/Indexed /DeviceRGB 3 <FF000000FF000000FFFFFFFF>] /Decode [3 0] >>").into_dict().unwrap();
    let object = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![0x18, 0xe4])))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, object)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    let expected = vec![vec![
        255, 255, 255, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0,
        255, 255,
    ]];
    let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
        let mut device = ColorDevice::default();
        doc.get_page(0).unwrap().display(&mut device).unwrap();
        assert_eq!(device.indexed_pixels, expected);
    };
    verify(&doc);
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
    }
}

#[test]
fn indexed_lab_and_icc_ranges_survive_resource_registration_and_save() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    for (icc, indexed, reverse) in [
        (false, true, false),
        (true, true, false),
        (false, false, false),
        (true, false, false),
        (false, false, true),
        (true, false, true),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let base = if icc {
            let dict = parse_object("<< /N 1 /Range [0.25 0.75] /Alternate /DeviceGray >>")
                .into_dict()
                .unwrap();
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![])))
                .unwrap();
            format!("[/ICCBased {} {} R]", id.obj_num(), id.gen_num())
        } else {
            "[/Lab << /WhitePoint [0.95047 1 1.08883] /Range [0 0 0 0] >>]".into()
        };
        let palette = if icc { "00FF" } else { "000000FFFFFF" };
        let (space, bits, samples) = if indexed {
            (format!("[/Indexed {base} 1 <{palette}>]"), 8, vec![0, 1])
        } else if icc {
            (base, 16, vec![0, 0, 255, 255])
        } else {
            (base, 16, vec![0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0])
        };
        let decode = if reverse {
            if icc {
                "/Decode [0.75 0.25]"
            } else {
                "/Decode [100 0 0 0 0 0]"
            }
        } else {
            ""
        };
        let dict = parse_object(&format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent {bits} /ColorSpace {space} {decode} >>")).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, samples)))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let mut expected = if icc {
            vec![vec![64, 64, 64, 255, 191, 191, 191, 255]]
        } else {
            vec![vec![0, 0, 0, 255, 255, 255, 255, 255]]
        };
        if reverse {
            expected[0].rotate_left(4);
        }
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let mut device = ColorDevice::default();
            doc.get_page(0).unwrap().display(&mut device).unwrap();
            assert_eq!(device.indexed_pixels, expected);
        };
        verify(&doc);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap(),
                ParseMode::Strict,
            )
            .unwrap();
            verify(&reopened);
        }
    }
}

impl ColorDevice {
    fn record(&mut self, path: &PathObject, stroke: bool) -> PdfResult<()> {
        let state = &path.graphic_state().color_state;
        let rgb = if stroke {
            state.stroke_color_space.rgb(&state.stroke_color_value)?
        } else {
            state.fill_color_space.rgb(&state.fill_color_value)?
        };
        self.colors.push([rgb.r(), rgb.g(), rgb.b()]);
        Ok(())
    }
}

impl Device for ColorDevice {
    fn do_shading(&mut self, shading: pdf::page::shading_object::ShadingObject) -> PdfResult<()> {
        self.shadings.push(shading);
        Ok(())
    }
    fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }
    fn do_image(&mut self, image: ImageObject) -> PdfResult<()> {
        self.images += 1;
        if matches!(
            image.image.colorspace(),
            pdf::color::colorspace::ColorSpace::Indexed(_)
                | pdf::color::colorspace::ColorSpace::CalGray(_)
                | pdf::color::colorspace::ColorSpace::CalRgb(_)
                | pdf::color::colorspace::ColorSpace::Lab(_)
                | pdf::color::colorspace::ColorSpace::IccBased(_)
                | pdf::color::colorspace::ColorSpace::Separation(_)
                | pdf::color::colorspace::ColorSpace::DeviceN(_)
        ) {
            self.indexed_pixels.push(image.image.rgba_buffer()?);
        }
        Ok(())
    }
    fn show_text(&mut self, text: TextObject) -> PdfResult<()> {
        self.text_modes
            .push(text.state.text_state.text_rendering_mode);
        Ok(())
    }
    fn clip_path(&mut self, _: PathObject) -> PdfResult<()> {
        Ok(())
    }
    fn fill_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.record(&path, false)
    }
    fn stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.record(&path, true)
    }
    fn fill_and_stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.record(&path, false)?;
        self.record(&path, true)
    }
}

fn document(black: &str, content: &[u8]) -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new().unwrap();
    let resources = SyntaxParser::new(StreamReader::try_new(Cursor::new(format!(
        "<< /ColorSpace << /Lab [/Lab << /WhitePoint [0.96429568 1 0.8251046] /BlackPoint [{black}] >>] /Spot [/Separation /Spot /DeviceRGB << /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [<< /FunctionType 2 /Domain [0 1] /N 1 /C0 [1 0 0] /C1 [0 1 0] >> << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0 0 1] /C1 [1 1 1] >>] >>] >> >>"
    ).into_bytes())).unwrap()).get_object().unwrap().into_dict().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new()
            .size(200., 200.)
            .resources(resources)
            .content(content.to_vec()),
    )
    .unwrap();
    doc
}

fn colors(doc: &PdfDocument<Cursor<Vec<u8>>>, index: u32) -> Vec<[u8; 3]> {
    let mut device = ColorDevice::default();
    doc.get_page(index)
        .unwrap()
        .display_list()
        .unwrap()
        .replay(&mut device)
        .unwrap();
    device.colors
}

#[test]
fn lab_and_stitching_page_colors_survive_graphics_restore_and_both_save_modes() {
    let doc = document("0 0 0", b"/Lab cs 50 0 0 sc 10 10 20 20 re f q /Spot cs 0.5 sc 40 10 20 20 re f Q 70 10 20 20 re f /Lab CS 100 0 0 SC 10 50 20 20 re S");
    let expected = vec![[119; 3], [0, 0, 255], [119; 3], [255; 3]];
    assert_eq!(colors(&doc, 0), expected);
    let stamp = doc.stamp();
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(colors(&reopened, 0), expected);
    }
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn editing_copied_lab_page_does_not_change_original_colors() {
    let doc = document("0 0 0", b"/Lab cs 50 0 0 sc 10 10 20 20 re f");
    doc.copy_page(0, 1).unwrap();
    doc.replace_page_contents(1, b"/Lab cs 100 0 0 sc 10 10 20 20 re f".to_vec())
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        assert_eq!(colors(doc, 0), vec![[119; 3]]);
        assert_eq!(colors(doc, 1), vec![[255; 3]]);
    }
}

#[test]
fn unsupported_lab_black_point_stops_color_device_replay_before_next_draw() {
    let doc = document(
        "0.01 0.01 0.01",
        b"/Lab cs 50 0 0 sc 10 10 20 20 re f /DeviceRGB cs 1 0 0 sc 40 10 20 20 re f",
    );
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let mut device = ColorDevice::default();
    assert!(matches!(
        list.replay(&mut device),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(device.colors.is_empty());
}

#[test]
fn updating_shared_source_function_refreshes_pages_but_not_old_display_lists() {
    let initial = PdfDocument::new().unwrap();
    let object = |source: &str| {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    };
    let function = |rgb: &str| {
        object(&format!(
            "<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [0 1] /Functions [<< /FunctionType 2 /Domain [0 1] /N 1 /C0 [{rgb}] /C1 [{rgb}] >>] >>"
        ))
    };
    let id = initial.try_insert_object(function("0 0 1")).unwrap();
    let resources = object(&format!(
        "<< /ColorSpace << /Spot [/Separation /Spot /DeviceRGB {} {} R] >> >>",
        id.obj_num(),
        id.gen_num()
    ))
    .into_dict()
    .unwrap();
    for index in 0..2 {
        initial
            .add_page_with_builder(
                index,
                PageBuilder::new()
                    .resources(resources.clone())
                    .content(b"/Spot cs 0.5 sc 10 10 20 20 re f".to_vec()),
            )
            .unwrap();
    }
    let doc =
        PdfDocument::from_bytes_with_mode(initial.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    let old_list = doc.get_page(0).unwrap().display_list().unwrap();
    assert_eq!(colors(&doc, 1), vec![[0, 0, 255]]);
    let old_stamp = doc.stamp();
    doc.update_object(id, function("1 0 0")).unwrap();
    assert!(!doc.is_stamp_current(&old_stamp));
    for index in 0..2 {
        assert_eq!(colors(&doc, index), vec![[255, 0, 0]]);
    }
    let mut old_device = ColorDevice::default();
    old_list.replay(&mut old_device).unwrap();
    assert_eq!(old_device.colors, vec![[0, 0, 255]]);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for index in 0..2 {
        assert_eq!(colors(&reopened, index), vec![[255, 0, 0]]);
    }
}

fn parse_object(source: &str) -> pdf::objects::PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}

#[test]
fn none_dispatch_preserves_objects_text_clipping_and_visible_sides_of_combined_paths() {
    use pdf::{
        document::resource_manager::{ResourceKind, StandardFont},
        objects::{PdfObject, pdf_stream::PdfStream},
        page::text_state::TextRenderingMode::*,
    };
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let none = doc
        .resources()
        .register_color_space(
            0,
            parse_object(
                "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            ),
        )
        .unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let image_dict = parse_object("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 >>").into_dict().unwrap();
    let mask = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(image_dict, vec![0])))
        .unwrap();
    let mask = doc
        .resources()
        .register_existing(0, ResourceKind::Image, mask)
        .unwrap();
    let content = format!(
        "/{none} cs 1 0 0 RG 0 0 20 20 re B 0 1 0 rg /{none} CS 20 0 20 20 re B /{none} cs 40 0 20 20 re B /{mask} Do 0 0 1 rg /{mask} Do /{none} cs 1 0 0 RG BT /{font} 10 Tf 1 0 0 1 0 50 Tm {} ET",
        (0..8)
            .map(|mode| format!("{mode} Tr (A) Tj"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    doc.replace_page_contents(0, content.into_bytes()).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let list = page.display_list().unwrap();
    assert_eq!(list.objects().len(), 13);
    assert_eq!(
        page.extract_characters_paint_order()
            .unwrap()
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "AAAAAAAA"
    );
    let mut device = ColorDevice::default();
    list.replay(&mut device).unwrap();
    assert_eq!(device.colors, vec![[255, 0, 0], [0, 255, 0]]);
    // Non-stencil images use their own color space, not the current fill color.
    assert_eq!(device.images, 2);
    assert_eq!(
        device.text_modes,
        vec![
            INVisible, Stroke, Stroke, INVisible, Clip, StrokeClip, StrokeClip, Clip
        ]
    );
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap(),
            ParseMode::Strict,
        )
        .unwrap();
        let mut replayed = ColorDevice::default();
        reopened
            .get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .replay(&mut replayed)
            .unwrap();
        assert_eq!(replayed.colors, device.colors);
        assert_eq!(replayed.images, device.images);
        assert_eq!(replayed.text_modes, device.text_modes);
    }
}

#[test]
fn separation_all_ignores_transform_and_none_does_not_paint_an_alternate_color() {
    use pdf::objects::{PdfObject, pdf_stream::PdfStream};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse_object("<< /FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1] >>")
        .into_dict()
        .unwrap();
    let transform = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, b"{}".to_vec())))
        .unwrap();
    let space = |name| {
        parse_object(&format!(
            "[/Separation /{name} /DeviceRGB {} {} R]",
            transform.obj_num(),
            transform.gen_num()
        ))
    };
    let all = doc
        .resources()
        .register_color_space(0, space("All"))
        .unwrap();
    let none = doc
        .resources()
        .register_color_space(0, space("None"))
        .unwrap();
    doc.replace_page_contents(0, format!("/{all} cs 0 scn 0 0 20 20 re f 0.5 scn 20 0 20 20 re f 1 scn 40 0 20 20 re f -1 scn 60 0 20 20 re f /{all} CS 2 SCN 0 30 20 20 re S").into_bytes()).unwrap();
    let expected = vec![[255; 3], [128; 3], [0; 3], [255; 3], [0; 3]];
    assert_eq!(colors(&doc, 0), expected);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(colors(&reopened, 0), expected);
    doc.replace_page_contents(
        0,
        format!("/{none} cs 0.5 scn 0 0 20 20 re f 1 0 0 rg 20 0 20 20 re f").into_bytes(),
    )
    .unwrap();
    let mut device = ColorDevice::default();
    doc.get_page(0)
        .unwrap()
        .display_list()
        .unwrap()
        .replay(&mut device)
        .unwrap();
    assert_eq!(device.colors, vec![[255, 0, 0]]);
}

#[test]
fn separation_registration_rejects_input_and_output_mismatch_without_partial_binding() {
    use pdf::objects::{PdfObject, pdf_stream::PdfStream};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse_object("<< /FunctionType 4 /Domain [0 1 0 1] /Range [0 1] >>")
        .into_dict()
        .unwrap();
    let transform = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, b"{}".to_vec())))
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    for source in [
        "[/Separation /Spot /DeviceRGB << /FunctionType 2 /Domain [0 1] /N 1 >>]".to_string(),
        "[/Separation /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0 0 0] /C1 [1 1 1] >>]".to_string(),
        format!("[/Separation /Spot /DeviceGray {} {} R]", transform.obj_num(), transform.gen_num()),
    ] {
        for _ in 0..2 {
            assert!(matches!(doc.resources().register_color_space(0, parse_object(&source)), Err(PdfError::ColorError(_))));
            assert_eq!(doc.save_to_bytes().unwrap(), before);
        }
    }
    let name = doc
        .resources()
        .register_color_space(
            0,
            parse_object(
                "[/Separation /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            ),
        )
        .unwrap();
    assert_eq!(name, "CS1");
    doc.replace_page_contents(0, format!("/{name} cs 0.5 scn 0 0 20 20 re f").into_bytes())
        .unwrap();
    assert_eq!(colors(&doc, 0), vec![[128; 3]]);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(colors(&reopened, 0), vec![[128; 3]]);
}

#[test]
fn devicen_two_tints_render_via_sampled_alternate_and_survive_both_save_modes() {
    use pdf::objects::{PdfObject, pdf_stream::PdfStream};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse_object(
        "<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 2] /BitsPerSample 8 >>",
    )
    .into_dict()
    .unwrap();
    let transform = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict,
            vec![255, 255, 255, 0, 255, 255, 255, 0, 255, 0, 0, 255],
        )))
        .unwrap();
    let space = parse_object(&format!(
        "[/DeviceN [/Cyan /Magenta] /DeviceRGB {} {} R]",
        transform.obj_num(),
        transform.gen_num()
    ));
    let name = doc.resources().register_color_space(0, space).unwrap();
    doc.replace_page_contents(0, format!("/{name} cs 0 30 20 20 re f 0 0 scn 0 0 20 20 re f 1 0 scn 20 0 20 20 re f 0 1 scn 40 0 20 20 re f -1 2 scn 60 0 20 20 re f").into_bytes()).unwrap();
    let expected = vec![
        [0, 0, 255],
        [255, 255, 255],
        [0, 255, 255],
        [255, 0, 255],
        [255, 0, 255],
    ];
    assert_eq!(colors(&doc, 0), expected);
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(colors(&reopened, 0), expected);
    }
}

#[test]
fn indexed_devicen_maps_all_palette_tints_including_mixed_none() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    for names in ["/Cyan /Magenta", "/Cyan /None"] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let dict = parse_object("<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 2] /BitsPerSample 8 >>").into_dict().unwrap();
        let transform = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                dict,
                vec![255, 255, 255, 0, 255, 255, 255, 0, 255, 0, 0, 255],
            )))
            .unwrap();
        let space = format!(
            "[/Indexed [/DeviceN [{names}] /DeviceRGB {} {} R] 3 <0000FF0000FFFFFF>]",
            transform.obj_num(),
            transform.gen_num()
        );
        let color = doc
            .resources()
            .register_color_space(0, parse_object(&space))
            .unwrap();
        let dict = parse_object(&format!("<< /Type /XObject /Subtype /Image /Width 4 /Height 1 /BitsPerComponent 2 /ColorSpace {space} >>")).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![0x1b])))
            .unwrap();
        let image = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{color} cs 0 sc 0 0 10 10 re f 1 sc 10 0 10 10 re f 2 sc 20 0 10 10 re f 3 sc 30 0 10 10 re f /{image} Do").into_bytes()).unwrap();
        let expected = vec![[255, 255, 255], [0, 255, 255], [255, 0, 255], [0, 0, 255]];
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let mut device = ColorDevice::default();
            doc.get_page(0).unwrap().display(&mut device).unwrap();
            assert_eq!(device.colors, expected);
            assert_eq!(
                device.indexed_pixels,
                vec![
                    expected
                        .iter()
                        .flat_map(|p| [p[0], p[1], p[2], 255])
                        .collect::<Vec<_>>()
                ]
            );
        };
        verify(&doc);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        }
    }
}

#[test]
fn indexed_none_bases_suppress_paths_and_images_without_losing_objects() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    for base in [
        "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/None] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let space = format!("[/Indexed {base} 1 <00FF>]");
        let color = doc
            .resources()
            .register_color_space(0, parse_object(&space))
            .unwrap();
        let dict = parse_object(&format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 1 /ColorSpace {space} >>")).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![0x40])))
            .unwrap();
        let image = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("/{color} cs 0 sc 0 0 10 10 re f /{image} Do 1 0 0 rg 10 0 10 10 re f")
                .into_bytes(),
        )
        .unwrap();
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 3);
            let mut device = ColorDevice::default();
            list.replay(&mut device).unwrap();
            assert_eq!(device.images, 0);
            assert_eq!(device.colors, vec![[255, 0, 0]]);
        };
        verify(&doc);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        }
    }
}

#[test]
fn devicen_registration_rejects_invalid_names_alternates_and_dimensions_atomically() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let before = doc.save_to_bytes().unwrap();
    for source in [
        "[/DeviceN [] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/All] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/Spot /Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [1] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/A /B] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/A] /DeviceRGB << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/A] /Pattern << /FunctionType 2 /Domain [0 1] /N 1 >>]",
        "[/DeviceN [/None] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >> << /Subtype /NChannel >>]",
        "[/DeviceN [/A] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >> 1]",
    ] {
        assert!(
            doc.resources()
                .register_color_space(0, parse_object(source))
                .is_err(),
            "{source}"
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
    let name = doc
        .resources()
        .register_color_space(
            0,
            parse_object("[/DeviceN [/None] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"),
        )
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} cs 0.5 scn 0 0 20 20 re f").into_bytes())
        .unwrap();
    let mut device = ColorDevice::default();
    doc.get_page(0)
        .unwrap()
        .display_list()
        .unwrap()
        .replay(&mut device)
        .unwrap();
    assert!(device.colors.is_empty());
    assert_eq!(
        doc.resources()
            .register_color_space(
                0,
                parse_object(
                    "[/DeviceN [/Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
                )
            )
            .unwrap(),
        "CS2"
    );
}

#[test]
fn color_registration_avoids_collisions_reuses_handles_and_isolates_shared_tables() {
    use pdf::document::resource_manager::ResourceKind;
    let doc = PdfDocument::new().unwrap();
    let table = doc
        .try_insert_object(parse_object("<< /CS1 /DeviceGray >>"))
        .unwrap();
    let resources = parse_object(&format!(
        "<< /ColorSpace {} {} R >>",
        table.obj_num(),
        table.gen_num()
    ))
    .into_dict()
    .unwrap();
    for index in 0..2 {
        doc.add_page_with_builder(index, PageBuilder::new().resources(resources.clone()))
            .unwrap();
    }
    let lab = doc
        .try_insert_object(parse_object("[/Lab << /WhitePoint [1 1 1] >>]"))
        .unwrap();
    let handle = doc.object_handle(lab).unwrap();
    let name = doc
        .resources()
        .register_handle(0, ResourceKind::ColorSpace, &handle)
        .unwrap();
    assert_eq!(name, "CS2");
    let stamp = doc.stamp();
    assert_eq!(
        doc.resources()
            .register_handle(0, ResourceKind::ColorSpace, &handle)
            .unwrap(),
        name
    );
    assert!(doc.is_stamp_current(&stamp));
    assert!(
        doc.resolve_object(table)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("CS2")
            .is_none()
    );
    doc.replace_page_contents(
        0,
        format!("/{name} cs 50 0 0 sc 10 10 20 20 re f").into_bytes(),
    )
    .unwrap();
    assert_eq!(colors(&doc, 0), vec![[119; 3]]);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(colors(&reopened, 0), vec![[119; 3]]);
}

#[test]
fn color_space_import_copies_indirect_graph_and_rejects_foreign_registration() {
    use pdf::document::resource_manager::ResourceKind;
    let source = PdfDocument::new().unwrap();
    let white = source
        .try_insert_object(parse_object("[0.96429568 1 0.8251046]"))
        .unwrap();
    let color = source
        .try_insert_object(parse_object(&format!(
            "[/Lab << /WhitePoint {} {} R >>]",
            white.obj_num(),
            white.gen_num()
        )))
        .unwrap();
    let handle = source.object_handle(color).unwrap();
    let source_bytes = source.save_to_bytes().unwrap();
    let source_stamp = source.stamp();
    let target = PdfDocument::new().unwrap();
    target.add_page(0).unwrap();
    let target_bytes = target.save_to_bytes().unwrap();
    assert!(
        target
            .resources()
            .register_handle(0, ResourceKind::ColorSpace, &handle)
            .is_err()
    );
    assert_eq!(target.save_to_bytes().unwrap(), target_bytes);
    let imported = target
        .import_resource_from(&source, &handle, ResourceKind::ColorSpace)
        .unwrap();
    let name = target
        .resources()
        .register_handle(0, ResourceKind::ColorSpace, &imported)
        .unwrap();
    target
        .replace_page_contents(
            0,
            format!("/{name} cs 50 0 0 sc 10 10 20 20 re f").into_bytes(),
        )
        .unwrap();
    assert_eq!(colors(&target, 0), vec![[119; 3]]);
    assert_eq!(source.save_to_bytes().unwrap(), source_bytes);
    assert!(source.is_stamp_current(&source_stamp));
    source
        .update_object(white, parse_object("[1 2 1]"))
        .unwrap();
    assert_eq!(colors(&target, 0), vec![[119; 3]]);
    let stamp = target.stamp();
    let same = target
        .import_resource_from(&target, &imported, ResourceKind::ColorSpace)
        .unwrap();
    assert_eq!(
        target
            .resources()
            .register_handle(0, ResourceKind::ColorSpace, &same)
            .unwrap(),
        name
    );
    assert!(target.is_stamp_current(&stamp));
    let reopened =
        PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert_eq!(colors(&reopened, 0), vec![[119; 3]]);
}

#[test]
fn color_registration_validation_and_outer_failure_preserve_page_resources() {
    use pdf::document::resource_manager::ResourceKind;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let invalid = doc.try_insert_object(parse_object("[/Lab << >>]")).unwrap();
    let good = doc.try_insert_object(parse_object("/DeviceRGB")).unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    assert!(
        doc.resources()
            .register_existing(0, ResourceKind::ColorSpace, invalid)
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        assert_eq!(
            doc.resources()
                .register_existing(0, ResourceKind::ColorSpace, good)?,
            "CS1"
        );
        Err(PdfError::DocumentError("outer failure".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::ColorSpace, good)
            .unwrap(),
        "CS1"
    );
}

#[test]
fn color_import_validation_and_mid_graph_limit_restore_objects_and_next_id() {
    use pdf::{
        document::resource_manager::ResourceKind,
        parser::options::{Limits, ParseOptions},
    };
    let source = PdfDocument::new().unwrap();
    let white = source.try_insert_object(parse_object("[1 1 1]")).unwrap();
    let color = source
        .try_insert_object(parse_object(&format!(
            "[/Lab << /WhitePoint {} {} R >>]",
            white.obj_num(),
            white.gen_num()
        )))
        .unwrap();
    let handle = source.object_handle(color).unwrap();
    let short = source
        .try_insert_object(parse_object("/DeviceRGB"))
        .unwrap();
    let short_handle = source.object_handle(short).unwrap();
    let target = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 4,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let before = target.save_to_bytes().unwrap();
    source
        .update_object(white, parse_object("[1 2 1]"))
        .unwrap();
    assert!(matches!(
        target.import_resource_from(&source, &handle, ResourceKind::ColorSpace),
        Err(PdfError::ColorError(_))
    ));
    assert_eq!(target.save_to_bytes().unwrap(), before);
    source
        .update_object(white, parse_object("[1 1 1]"))
        .unwrap();
    let source_bytes = source.save_to_bytes().unwrap();
    for _ in 0..2 {
        assert!(matches!(
            target.import_resource_from(&source, &handle, ResourceKind::ColorSpace),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(target.save_to_bytes().unwrap(), before);
        assert_eq!(source.save_to_bytes().unwrap(), source_bytes);
    }
    let imported = target
        .import_resource_from(&source, &short_handle, ResourceKind::ColorSpace)
        .unwrap();
    assert_eq!(imported.object_id().obj_num(), 3);
    assert_eq!(
        target.resolve_handle(&imported).unwrap(),
        parse_object("/DeviceRGB")
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert_eq!(
        reopened.resolve_object(imported.object_id()).unwrap(),
        parse_object("/DeviceRGB")
    );
}

#[test]
fn owned_color_registration_reuses_values_and_rolls_back_failed_creation() {
    use pdf::objects::{PdfObject, pdf_reference::PdfReference};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let value = parse_object("[/Lab << /WhitePoint [1 1 1] >>]");
    let name = doc
        .resources()
        .register_color_space(0, value.clone())
        .unwrap();
    assert_eq!(name, "CS1");
    let stamp = doc.stamp();
    let bytes = doc.save_to_bytes().unwrap();
    assert_eq!(
        doc.resources()
            .register_color_space(0, value.clone())
            .unwrap(),
        name
    );
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    assert!(
        doc.resources()
            .register_color_space(0, parse_object("[/Lab << >>]"))
            .is_err()
    );
    assert!(doc.resources().register_color_space(10, value).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        assert_eq!(
            doc.resources()
                .register_color_space(0, parse_object("/DeviceRGB"))?,
            "CS2"
        );
        Err(PdfError::DocumentError("outer failure".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    assert_eq!(
        doc.resources()
            .register_color_space(0, parse_object("/DeviceRGB"))
            .unwrap(),
        "CS2"
    );
    let id = doc.try_insert_object(parse_object("/DeviceRGB")).unwrap();
    let stamp = doc.stamp();
    assert_eq!(
        doc.resources()
            .register_color_space(0, PdfObject::PdfReference(PdfReference::from_id(id)))
            .unwrap(),
        "CS2"
    );
    assert!(doc.is_stamp_current(&stamp));
    doc.replace_page_contents(
        0,
        format!("/{name} cs 50 0 0 sc 10 10 20 20 re f").into_bytes(),
    )
    .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(colors(&reopened, 0), vec![[119; 3]]);
}

#[test]
fn color_value_reuse_succeeds_at_object_ceiling_without_partial_new_binding() {
    use pdf::{
        objects::PdfObject,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 16,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    assert_eq!(
        doc.resources()
            .register_color_space(0, parse_object("/DeviceRGB"))
            .unwrap(),
        "CS1"
    );
    let mut filled = false;
    for _ in 0..16 {
        match doc.try_insert_object(PdfObject::PdfNull) {
            Ok(_) => {}
            Err(PdfError::ResourceLimit(_)) => {
                filled = true;
                break;
            }
            other => panic!("unexpected allocation result: {other:?}"),
        }
    }
    assert!(filled);
    let before = doc.get_page(0).unwrap().resource().unwrap();
    for _ in 0..2 {
        let stamp = doc.stamp();
        assert_eq!(
            doc.resources()
                .register_color_space(0, parse_object("/DeviceRGB"))
                .unwrap(),
            "CS1"
        );
        assert!(doc.is_stamp_current(&stamp));
        assert!(matches!(
            doc.resources()
                .register_color_space(0, parse_object("/DeviceGray")),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), before);
        assert_eq!(
            doc.get_page(0).unwrap().get_resource_color("CS1").unwrap(),
            parse_object("/DeviceRGB")
        );
    }
    assert_eq!(
        doc.resources()
            .register_color_space(0, parse_object("/DeviceRGB"))
            .unwrap(),
        "CS1"
    );
}

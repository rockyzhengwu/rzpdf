use super::*;
use pdf::{
    color::icc_profile::ICCProfile,
    document::{Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn object(text: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}

// Same primaries as sRGB, but linear light. A 50% black/white mixture must
// emerge as approximately 188 in sRGB, not 128. No external/private fixture.
fn linear_profile() -> Vec<u8> {
    let mut profile = moxcms::ColorProfile::new_srgb();
    let linear = moxcms::ColorProfile::new_gray_with_gamma(1.).gray_trc;
    profile.red_trc = linear.clone();
    profile.green_trc = linear.clone();
    profile.blue_trc = linear;
    profile.encode().unwrap()
}

fn document(inner_space: Option<&str>, half_group: bool) -> Document {
    document_sized(inner_space, half_group, (30., 20.))
}
fn document_sized(inner_space: Option<&str>, half_group: bool, size: (f32, f32)) -> Document {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new().size(size.0, size.1).resources(
            object("<< /ExtGState << /Half << /ca 0.5 >> >> >>")
                .into_dict()
                .unwrap(),
        ),
    )
    .unwrap();
    let profile = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                object("<< /N 3 >>").into_dict().unwrap(),
                linear_profile(),
            )
            .unwrap(),
        ))
        .unwrap();
    // Both the CS array and profile stream are indirect, as in the reported PDF.
    let cs = doc
        .try_insert_object(object(&format!("[/ICCBased {} 0 R]", profile.obj_num())))
        .unwrap();
    let paints = b"0 0 0 rg 0 0 20 20 re f /Half gs 1 1 1 rg 0 0 20 20 re f";
    let resources = " /ExtGState << /Half << /ca 0.5 >> >>";
    let body = if let Some(space) = inner_space {
        let inner = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            object(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Group << /S /Transparency {space} >> /Resources << {resources} >> >>")).into_dict().unwrap(), paints.to_vec()).unwrap())).unwrap();
        (
            b"/Inner Do".to_vec(),
            format!("{resources} /XObject << /Inner {} 0 R >>", inner.obj_num()),
        )
    } else {
        (paints.to_vec(), resources.to_owned())
    };
    let form = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        object(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Group << /S /Transparency /I true /CS {} 0 R >> /Resources << {} >> >>", cs.obj_num(), body.1)).into_dict().unwrap(), body.0).unwrap())).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    let alpha = if half_group { "/Half gs" } else { "" };
    doc.replace_page_contents(
        0,
        format!("0 1 0 rg 0 0 30 20 re f q {alpha} /{name} Do Q 0.5 0.5 0.5 rg 25 0 5 20 re f")
            .into_bytes(),
    )
    .unwrap();
    doc
}

fn pixel(renderer: &mut SkiaRender, x: usize) -> [u8; 4] {
    let info = skia_safe::ImageInfo::new(
        (30, 20),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut bytes = vec![0; 30 * 20 * 4];
    assert!(renderer.surface.read_pixels(&info, &mut bytes, 120, (0, 0)));
    bytes[(10 * 30 + x) * 4..(10 * 30 + x) * 4 + 4]
        .try_into()
        .unwrap()
}

#[test]
fn icc_group_blends_before_converting_and_nested_device_rgb_restores_parent() {
    for (inner, expected) in [
        (None, 188),
        (Some("/I false"), 188),
        (Some("/I true /CS /DeviceRGB"), 128),
    ] {
        let doc = document(inner, false);
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let mut render = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(0).unwrap().display(&mut render).unwrap();
            let actual = pixel(&mut render, 10);
            assert!(
                actual[..3]
                    .iter()
                    .all(|v| (*v as i32 - expected).abs() <= 3),
                "{inner:?}: {actual:?}"
            );
            assert_eq!(actual[3], 255);
            assert!(
                pixel(&mut render, 27)[..3]
                    .iter()
                    .all(|v| (*v as i32 - 128).abs() <= 1)
            );
            assert!(render.color_context.output_profile().is_builtin_srgb());
            assert!(render.group_stack.is_empty());
        }
    }
}

#[test]
fn icc_group_opacity_is_applied_in_parent_space_after_conversion() {
    let doc = document(None, true);
    let mut render = SkiaRender::new(72., 72.).unwrap();
    doc.get_page(0).unwrap().display(&mut render).unwrap();
    let actual = pixel(&mut render, 10);
    for (actual, expected) in actual.into_iter().zip([94, 222, 94, 255]) {
        assert!((actual as i32 - expected).abs() <= 3);
    }
}

#[test]
fn failed_group_restores_colour_context_without_committing_pixels() {
    let mut render = SkiaRender::new(72., 72.).unwrap();
    render.start_page(30., 20.).unwrap();
    let profile = ICCProfile::try_new(&linear_profile()).unwrap();
    render
        .begin_group(pdf::page::transparency::GroupPaint {
            blending_profile: Some(profile),
            rendering_intent: Default::default(),
            isolated: true,
            knockout: false,
            alpha: 1.,
            blend_mode: Default::default(),
            clip: Default::default(),
            soft_mask: None,
        })
        .unwrap();
    assert!(!render.color_context.output_profile().is_builtin_srgb());
    render.end_group(false).unwrap();
    assert!(render.color_context.output_profile().is_builtin_srgb());
    assert_eq!(pixel(&mut render, 10), [0; 4]);
}

#[test]
fn small_icc_and_nested_groups_fit_browser_limits_on_a_large_page() {
    let limits = Limits {
        max_render_pixels: 4_000_000,
        max_render_bytes: 16_000_000,
        ..Default::default()
    };
    for inner in [None, Some("/I false"), Some("/I true /CS /DeviceRGB")] {
        let doc = document_sized(inner, false, (600., 840.));
        let page = doc.get_page(0).unwrap().display_list().unwrap();
        let mut render = SkiaRender::with_limits(144., 144., limits).unwrap();
        page.replay(&mut render).unwrap();
        assert_eq!((render.page_width, render.page_height), (1200, 1680));
        assert!(render.group_stack.is_empty());
        assert!(render.region.is_none());
        assert_eq!(render.limits.max_render_bytes, limits.max_render_bytes);
        let image = render.to_rgba().unwrap();
        let offset = ((1680 - 20) * 1200 + 20) * 4;
        let expected = if inner == Some("/I true /CS /DeviceRGB") {
            128
        } else {
            188
        };
        assert!(
            image.pixels[offset..offset + 3]
                .iter()
                .all(|v| (i32::from(*v) - expected).abs() <= 3)
        );
        let whole = image.pixels;
        let region = RasterRegion {
            x: 0,
            y: 1620,
            width: 96,
            height: 60,
        };
        render.set_region(Some(region)).unwrap();
        page.replay(&mut render).unwrap();
        let tile = render.to_rgba().unwrap();
        for y in 0..60 {
            for x in 0..96 {
                assert_eq!(
                    &tile.pixels[(y * 96 + x) * 4..(y * 96 + x + 1) * 4],
                    &whole[((y + 1620) * 1200 + x) * 4..((y + 1620) * 1200 + x + 1) * 4]
                );
            }
        }
    }
}

#[test]
fn local_group_color_conversion_rejection_restores_parent_geometry_and_retries() {
    let doc = document_sized(None, false, (600., 840.));
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let root = 1200 * 1680 * 4;
    let mut render = SkiaRender::with_limits(
        144.,
        144.,
        Limits {
            max_render_pixels: 4_000_000,
            max_render_bytes: root + 44 * 42 * 4,
            ..Default::default()
        },
    )
    .unwrap();
    let error = list.replay(&mut render).unwrap_err();
    assert!(
        matches!(
            error,
            PdfError::ResourceLimitExceeded {
                reason: "render_live_bytes",
                stage: "group_color_conversion",
                ..
            }
        ),
        "{error:?}"
    );
    assert!(render.group_stack.is_empty());
    assert_eq!((render.page_width, render.page_height), (1200, 1680));
    assert!(render.region.is_none());
    render.limits.max_render_bytes = 16_000_000;
    list.replay(&mut render).unwrap();
    assert!(render.group_stack.is_empty());
}

#[test]
fn rotated_group_bounds_are_geometric_even_when_skia_uses_a_shader_clip() {
    let doc = document_sized(None, false, (600., 840.));
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let name = resources
        .get("XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .0;
    doc.replace_page_contents(
        0,
        format!("q 0.8660254 0.5 -0.5 0.8660254 150 700 cm /{name} Do Q").into_bytes(),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let group = list
        .objects()
        .iter()
        .find_map(|object| {
            if let pdf::page::page_object::PageObject::GroupBegin(paint) = object {
                Some(paint)
            } else {
                None
            }
        })
        .unwrap();
    let mut render = SkiaRender::with_limits(
        144.,
        144.,
        Limits {
            max_render_pixels: 4_000_000,
            max_render_bytes: 16_000_000,
            ..Default::default()
        },
    )
    .unwrap();
    render.start_page(600., 840.).unwrap();
    let target = render.group_target(group).unwrap();
    assert!(
        target.width < 64 && target.height < 64,
        "{}x{}",
        target.width,
        target.height
    );
    assert!(
        render.clip_images.is_empty(),
        "sizing must not rasterize the clip"
    );
    list.replay(&mut render).unwrap();
    assert!(render.group_stack.is_empty());
    assert!(render.region.is_none());
}

use super::*;
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn document(bad: bool) -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new().unwrap();
    let resources = parse("<< /ExtGState << /Half << /ca 0.5 >> >> >>")
        .into_dict()
        .unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(30., 20.).resources(resources))
        .unwrap();
    let dict=parse("<< /Subtype /Form /BBox [0 0 20 20] /Resources << /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >> /Group << /S /Transparency /I true /K false /CS /DeviceRGB >> >>").into_dict().unwrap();
    let mut data =
        b"1 0 0 rg 0 0 20 20 re f 0 0 1 rg 10 0 10 20 re f BT /F 10 Tf 3 Tr (A) Tj ET".to_vec();
    if bad {
        data.extend_from_slice(b" /Missing Do");
    }
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, data).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("0 1 0 rg 0 0 30 20 re f q /Half gs /{name} Do Q 1 1 0 rg 25 0 5 20 re f")
            .into_bytes(),
    )
    .unwrap();
    doc
}
fn pixels(render: &mut SkiaRender) -> Vec<u8> {
    let info = skia_safe::ImageInfo::new(
        (30, 20),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; 30 * 20 * 4];
    assert!(render.surface.read_pixels(&info, &mut pixels, 120, (0, 0)));
    pixels
}
#[test]
fn knockout_erases_shape_even_when_member_opacity_is_zero() {
    for isolated in [true, false] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30., 20.))
            .unwrap();
        let dict=parse(&format!("<< /Subtype /Form /BBox [0 0 30 20] /Group << /S /Transparency /I {isolated} /K true >> /Resources << /ExtGState << /Half << /ca 0.5 >> /Zero << /ca 0 >> >> >> >>")).into_dict().unwrap();
        let id=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dict,b"1 0 0 rg 0 0 30 20 re f q /Half gs 0 0 1 rg 10 0 10 20 re f Q /Zero gs 20 0 10 20 re f".to_vec()).unwrap())).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("0 1 0 rg 0 0 30 20 re f /{name} Do").into_bytes(),
        )
        .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let mut render = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(0).unwrap().display(&mut render).unwrap();
            let data = pixels(&mut render);
            for (x, want) in [
                (5, [255, 0, 0, 255]),
                (15, [0, 128, 128, 255]),
                (25, [0, 255, 0, 255]),
            ] {
                let pixel = &data[(10 * 30 + x) * 4..(10 * 30 + x) * 4 + 4];
                assert!(
                    pixel
                        .iter()
                        .zip(want)
                        .all(|(a, b)| (*a as i32 - b).abs() <= 2),
                    "I={isolated} x={x} {pixel:?}"
                );
            }
            assert!(render.group_stack.is_empty());
            assert_eq!(render.surface.canvas().save_count(), 1);
        }
    }
}
#[test]
fn combined_fill_stroke_uses_one_implicit_knockout_object() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new().size(30., 20.).resources(
            parse("<< /ExtGState << /H << /ca 0.5 /CA 0.25 >> >> >>")
                .into_dict()
                .unwrap(),
        ),
    )
    .unwrap();
    doc.replace_page_contents(
        0,
        b"0 1 0 rg 0 0 30 20 re f /H gs 1 0 0 rg 0 0 1 RG 6 w 5 5 20 10 re B".to_vec(),
    )
    .unwrap();
    let mut render = SkiaRender::new(72., 72.).unwrap();
    doc.get_page(0).unwrap().display(&mut render).unwrap();
    let data = pixels(&mut render);
    for (x, want) in [
        (6, [0, 191, 64, 255]),
        (15, [128, 128, 0, 255]),
        (29, [0, 255, 0, 255]),
    ] {
        let pixel = &data[(10 * 30 + x) * 4..(10 * 30 + x) * 4 + 4];
        assert!(
            pixel
                .iter()
                .zip(want)
                .all(|(a, b)| (*a as i32 - b).abs() <= 2),
            "x={x}: {pixel:?}"
        );
    }
}
#[test]
fn isolated_group_applies_alpha_once_and_replays_without_losing_text() {
    let doc = document(false);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(
            list.objects()
                .iter()
                .filter(|o| matches!(o, PageObject::GroupBegin(_)))
                .count(),
            1
        );
        assert_eq!(
            list.objects()
                .iter()
                .filter(|o| matches!(o, PageObject::GroupEnd))
                .count(),
            1
        );
        let chars = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].unicode.as_deref(), Some("A"));
        let mut direct = SkiaRender::new(72., 72.).unwrap();
        doc.get_page(0).unwrap().display(&mut direct).unwrap();
        let mut replay = SkiaRender::new(72., 72.).unwrap();
        list.replay(&mut replay).unwrap();
        let actual = pixels(&mut direct);
        assert_eq!(actual, pixels(&mut replay));
        for (x, expected) in [
            (5, [128, 128, 0, 255]),
            (15, [0, 128, 128, 255]),
            (22, [0, 255, 0, 255]),
            (27, [255, 255, 0, 255]),
        ] {
            let pixel = &actual[(10 * 30 + x) * 4..(10 * 30 + x) * 4 + 4];
            // An offscreen 8-bit raster plus final group alpha introduces two
            // quantization stages. The independent gate retains this bound.
            assert!(
                pixel
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (*a as i32 - b).abs() <= 2),
                "{x}: {pixel:?}"
            );
        }
        assert_eq!(direct.surface.canvas().save_count(), 1);
    }
}
#[test]
fn child_failure_discards_group_and_restores_canvas_and_limits() {
    let doc = document(false);
    let mut render = SkiaRender::new(72., 72.).unwrap();
    let limit = render.limits.max_render_bytes;
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    // Remove the end marker to force a backend replay failure after paints.
    // Source interpretation errors instead fail before PdfPage::display starts
    // the renderer, which is checked separately below.
    let list = pdf::page::display_list::DisplayList::new(
        30.,
        20.,
        list.into_objects()
            .into_iter()
            .filter(|o| !matches!(o, PageObject::GroupEnd))
            .collect(),
    );
    assert!(list.replay(&mut render).is_err());
    let actual = pixels(&mut render);
    let histogram = actual.as_chunks::<4>().0.iter().fold(
        std::collections::BTreeMap::new(),
        |mut map, pixel| {
            *map.entry(pixel.to_vec()).or_insert(0usize) += 1;
            map
        },
    );
    assert!(
        actual
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 255, 0, 255]),
        "{histogram:?}"
    );
    assert!(render.group_stack.is_empty());
    assert_eq!(render.limits.max_render_bytes, limit);
    assert_eq!(render.surface.canvas().save_count(), 1);
    document(false)
        .get_page(0)
        .unwrap()
        .display(&mut render)
        .unwrap();
    let before = pixels(&mut render);
    assert!(
        document(true)
            .get_page(0)
            .unwrap()
            .display(&mut render)
            .is_err()
    );
    assert_eq!(pixels(&mut render), before);
}
#[test]
fn group_surface_budget_counts_parent_and_retries_cleanly() {
    let doc = document(false);
    let peak = 30 * 20 * 4 * 2;
    for allowance in [peak - 1, peak] {
        let mut render = SkiaRender::with_limits(
            72.,
            72.,
            Limits {
                max_render_bytes: allowance,
                ..Limits::default()
            },
        )
        .unwrap();
        let result = doc.get_page(0).unwrap().display(&mut render);
        if allowance < peak {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            assert!(render.group_stack.is_empty());
        } else {
            result.unwrap();
            assert!(render.group_stack.is_empty());
        }
    }
}

#[test]
fn nonisolated_group_uses_backdrop_then_removes_it_before_group_opacity() {
    for isolated in [false, true] {
        let doc = PdfDocument::new().unwrap();
        let resources = parse("<< /ExtGState << /Half << /ca 0.5 >> >> >>")
            .into_dict()
            .unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30., 20.).resources(resources))
            .unwrap();
        let dict=parse(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Resources << /ExtGState << /Multiply << /BM /Multiply >> >> >> /Group << /S /Transparency /I {isolated} >> >>")).into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    dict,
                    b"/Multiply gs 0.5 0.5 0.5 rg 0 0 20 20 re f".to_vec(),
                )
                .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("0 1 0 rg 0 0 30 20 re f /Half gs /{name} Do").into_bytes(),
        )
        .unwrap();
        let mut render = SkiaRender::new(72., 72.).unwrap();
        doc.get_page(0).unwrap().display(&mut render).unwrap();
        let actual = pixels(&mut render);
        let pixel = &actual[(10 * 30 + 5) * 4..(10 * 30 + 5) * 4 + 4];
        let expected = if isolated {
            [64, 191, 64, 255]
        } else {
            [0, 191, 0, 255]
        };
        assert!(
            pixel
                .iter()
                .zip(expected)
                .all(|(a, b)| (*a as i32 - b).abs() <= 2),
            "isolated {isolated}: {pixel:?}"
        );
        assert!(render.group_stack.is_empty());
    }
}

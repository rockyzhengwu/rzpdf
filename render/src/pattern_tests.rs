use super::*;
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn document(extra: &str, cell: &str, shape: &str) -> PdfDocument<Cursor<Vec<u8>>> {
    document_sized(40., extra, cell, shape)
}
fn document_sized(size: f32, extra: &str, cell: &str, shape: &str) -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(size, size))
        .unwrap();
    let dict=parse(&format!("<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] /XStep 10 /YStep 10 /Resources << >> {extra} >>")).into_dict().unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, cell.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/Pattern cs /Pattern CS /{name} scn /{name} SCN {shape}").into_bytes(),
    )
    .unwrap();
    doc
}
fn rgba(renderer: &mut SkiaRender) -> Vec<u8> {
    let info = skia_safe::ImageInfo::new(
        (40, 40),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0; 40 * 40 * 4];
    assert!(
        renderer
            .surface
            .read_pixels(&info, &mut pixels, 160, (0, 0))
    );
    pixels
}
#[test]
fn internal_multiply_reads_current_backdrop_at_nonuniform_dpi() {
    let doc = document(
        "/BBox [0 0 10 10] /Resources << /ExtGState << /M << /BM /Multiply >> >> >>",
        "/M gs 0.5 g 0 0 10 10 re f",
        "0 1 0 rg 0 0 40 40 re f /Pattern cs /P1 scn 0 0 40 40 re f",
    );
    let mut render = SkiaRender::new(144., 96.).unwrap();
    doc.get_page(0).unwrap().display(&mut render).unwrap();
    let info = skia_safe::ImageInfo::new(
        (80, 53),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut data = vec![0; 80 * 53 * 4];
    assert!(render.surface.read_pixels(&info, &mut data, 320, (0, 0)));
    for y in 2..51 {
        for x in 2..78 {
            assert_eq!(
                &data[(y * 80 + x) * 4..(y * 80 + x) * 4 + 4],
                [0, 128, 0, 255],
                "{x},{y}"
            );
        }
    }
}
#[test]
fn tiling_fill_pixels_and_q_restore_survive_save() {
    for step in ["", "/XStep -10 /YStep -10"] {
        let doc = document(
            step,
            "1 0 0 rg 0 0 4 4 re f",
            "q 1 0 0 1 3 5 cm 0 0 30 30 re f Q 0 1 0 rg 35 0 5 5 re f",
        );
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let mut reference = None;
        for doc in [&doc, &reopened] {
            let mut render = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(0).unwrap().display(&mut render).unwrap();
            let pixels = rgba(&mut render);
            let pixel = |x: usize, y: usize| &pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
            assert_eq!(pixel(11, 28), [255, 0, 0, 255]); // user 11,11, lattice unaffected by cm.
            assert_eq!(pixel(16, 28), [255, 255, 255, 255]);
            assert_eq!(pixel(37, 37), [0, 255, 0, 255]);
            if let Some(expected) = &reference {
                assert_eq!(&pixels, expected);
            } else {
                reference = Some(pixels);
            }
        }
    }
}
#[test]
fn pattern_strokes_and_transformed_clip_use_page_lattice() {
    let doc = document("", "0 0 1 rg 0 0 4 4 re f", "4 w 2 2 30 30 re S");
    let mut render = SkiaRender::new(72., 72.).unwrap();
    doc.get_page(0).unwrap().display(&mut render).unwrap();
    let pixels = rgba(&mut render);
    let pixel = |x: usize, y: usize| &pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
    assert_eq!(pixel(11, 37), [0, 0, 255, 255]);
    assert_eq!(pixel(16, 37), [255, 255, 255, 255]);
    assert_eq!(pixel(11, 28), [255, 255, 255, 255]);
}
#[test]
fn pattern_surface_budget_rejects_before_paint_and_renderer_recovers() {
    let doc = document("", "1 0 0 rg 0 0 4 4 re f", "0 0 40 40 re f");
    let mut render = SkiaRender::with_limits(
        72.,
        72.,
        Limits {
            max_render_bytes: 40 * 40 * 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        doc.get_page(0).unwrap().display(&mut render),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(
        rgba(&mut render)
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 255, 255, 255])
    );
    doc.replace_page_contents(0, b"0 1 0 rg 0 0 40 40 re f".to_vec())
        .unwrap();
    doc.get_page(0).unwrap().display(&mut render).unwrap();
    assert!(
        rgba(&mut render)
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 255, 0, 255])
    );
}

#[test]
fn combined_fill_and_pattern_stroke_preflight_prevents_partial_ink() {
    let doc = document("", "1 0 0 rg 0 0 4 4 re f", "0 1 0 rg 4 w 5 5 20 20 re B");
    let mut render = SkiaRender::with_limits(
        72.,
        72.,
        Limits {
            max_render_bytes: 40 * 40 * 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        doc.get_page(0).unwrap().display(&mut render),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(
        rgba(&mut render)
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 255, 255, 255])
    );
    assert_eq!(render.surface.canvas().save_count(), 1);
}

#[test]
fn nonuniform_overlapping_tiles_have_no_white_seams_and_charge_reduction_buffers() {
    let doc = document_sized(
        60.,
        "/BBox [0 0 14 14]",
        "1 0 0 rg 0 0 14 7 re f 0 0 1 rg 0 7 14 7 re f",
        "0 0 60 60 re f",
    );
    let peak = 120 * 80 * 4 * 12;
    for limit in [peak - 1, peak] {
        let mut render = SkiaRender::with_limits(
            144.,
            96.,
            Limits {
                max_render_bytes: limit,
                ..Limits::default()
            },
        )
        .unwrap();
        let result = doc.get_page(0).unwrap().display(&mut render);
        if limit < peak {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        } else {
            result.unwrap();
            let info = skia_safe::ImageInfo::new(
                (120, 80),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0u8; 120 * 80 * 4];
            assert!(
                render
                    .surface
                    .read_pixels(&info, &mut pixels, 120 * 4, (0, 0))
            );
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|pixel| pixel[1] == 0 && pixel[3] == 255)
            );
        }
    }
}

use super::*;
use pdf::{
    document::PdfDocument,
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
fn document(
    kind: &str,
    background: &str,
    transfer: &str,
    shape: &str,
) -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(40., 20.))
        .unwrap();
    let dict=parse("<< /Subtype /Form /BBox [0 0 30 20] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources << /ExtGState << /Half << /ca 0.5 >> >> >> >>").into_dict().unwrap();
    let group = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                dict,
                b"1 0 0 rg 0 0 10 20 re f /Half gs 0 1 0 rg 10 0 10 20 re f".to_vec(),
            )
            .unwrap(),
        ))
        .unwrap();
    let state = parse(&format!(
        "<< /Type /ExtGState /SMask << /Type /Mask /S /{kind} /G {} 0 R {background} {transfer} >> >>",
        group.obj_num()
    ));
    let id = doc.try_insert_object(state).unwrap();
    // ExtGState has no public registration kind yet; install its page-local
    // dictionary alongside a standard font through the existing page builder.
    let font = doc
        .resources()
        .register_standard_font(0, pdf::document::resource_manager::StandardFont::Courier)
        .unwrap();
    let page = doc.get_page(0).unwrap();
    let mut resources = page.resource().unwrap();
    let entries = parse(&format!(
        "<< /M {} 0 R /Clear << /SMask /None >> >>",
        id.obj_num()
    ));
    resources.insert("ExtGState".into(), entries);
    doc.add_page_with_builder(
        1,
        PageBuilder::new()
            .size(40., 20.)
            .resources(resources)
            .content(
                format!("0 0 1 rg {shape}")
                    .replace("FONT", &font)
                    .into_bytes(),
            ),
    )
    .unwrap();
    doc
}
fn pixels(render: &mut SkiaRender) -> Vec<u8> {
    let info = skia_safe::ImageInfo::new(
        (40, 20),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut data = vec![0u8; 40 * 20 * 4];
    assert!(render.surface.read_pixels(&info, &mut data, 160, (0, 0)));
    data
}
#[test]
fn alpha_luminosity_background_and_transfer_survive_strict_reopen() {
    for (kind, background, transfer, expected) in [
        ("Alpha", "", "", [0, 127, 255, 255]),
        (
            "Alpha",
            "",
            "/TR << /FunctionType 2 /Domain [0 1] /N 2 >>",
            [0, 191, 255, 255],
        ),
        ("Luminosity", "", "", [179, 180, 255, 255]),
        ("Luminosity", "/BC [1 1 1]", "", [179, 52, 0, 0]),
    ] {
        let doc = document(kind, background, transfer, "/M gs 0 0 40 20 re f");
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let mut render = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(1).unwrap().display(&mut render).unwrap();
            let data = pixels(&mut render);
            for (x, expected) in [5, 15, 25, 35].into_iter().zip(expected) {
                let pixel = &data[(10 * 40 + x) * 4..(10 * 40 + x) * 4 + 4];
                assert!(
                    (pixel[0] as i32 - expected).abs() <= 2
                        && (pixel[1] as i32 - expected).abs() <= 2
                        && pixel[2] == 255
                        && pixel[3] == 255,
                    "{kind} {background} {transfer} x{x}: {pixel:?}, {expected}"
                );
            }
        }
    }
}
#[test]
fn mask_is_fixed_at_gs_matrix_and_none_and_q_restore_it() {
    let doc = document(
        "Alpha",
        "",
        "",
        "q 1 0 0 1 5 0 cm /M gs 1 0 0 1 -5 0 cm 0 0 40 10 re f /Clear gs 0 10 10 10 re f Q 30 10 10 10 re f",
    );
    let mut render = SkiaRender::new(72., 72.).unwrap();
    doc.get_page(1).unwrap().display(&mut render).unwrap();
    let data = pixels(&mut render);
    for (x, y, blue) in [
        (2, 15, false),
        (7, 15, true),
        (2, 5, true),
        (35, 5, true),
        (35, 15, false),
    ] {
        let pixel = &data[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
        assert_eq!(
            pixel,
            if blue {
                [0, 0, 255, 255]
            } else {
                [255, 255, 255, 255]
            }
        );
    }
}
#[test]
fn soft_mask_surface_neighbor_budget_rejects_without_ink_and_recovers() {
    let doc = document("Alpha", "", "", "/M gs 0 0 40 20 re f");
    let peak = 40 * 20 * 4 * 4;
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
        let result = doc.get_page(1).unwrap().display(&mut render);
        if allowance < peak {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            assert!(
                pixels(&mut render)
                    .chunks_exact(4)
                    .all(|p| p == [255, 255, 255, 255])
            );
        } else {
            result.unwrap();
        }
    }
}

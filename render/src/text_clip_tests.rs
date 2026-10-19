use super::*;
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::ParseMode,
};
use std::io::Cursor;
fn document(content: &str) -> PdfDocument<Cursor<Vec<u8>>> {
    let source = PdfDocument::from_bytes_with_mode(
        include_bytes!("../../pdf/tests/fixtures/v07/fonts.pdf").to_vec(),
        ParseMode::Strict,
    )
    .unwrap();
    let resources = source.get_page(2).unwrap().resource().unwrap();
    let id = resources
        .get("Font")
        .unwrap()
        .get_attr("F0")
        .unwrap()
        .as_object_id()
        .unwrap();
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
        .unwrap();
    let handle = doc
        .import_resource_from(
            &source,
            &source.object_handle(id).unwrap(),
            ResourceKind::Font,
        )
        .unwrap();
    let font = doc
        .resources()
        .register_handle(0, ResourceKind::Font, &handle)
        .unwrap();
    doc.replace_page_contents(0, content.replace("FONT", &font).into_bytes())
        .unwrap();
    doc
}
fn pixels(render: &mut SkiaRender) -> Vec<u8> {
    let info = skia_safe::ImageInfo::new(
        (40, 40),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut data = vec![0u8; 40 * 40 * 4];
    assert!(render.surface.read_pixels(&info, &mut data, 160, (0, 0)));
    data
}
#[test]
fn text_clip_accumulates_across_shows_and_applies_only_at_et_then_q_restores() {
    let doc = document(
        "q BT /FONT 20 Tf 7 Tr 1 0 0 1 5 5 Tm (A) Tj 1 0 0 1 25 5 Tm (A) Tj ET 1 0 0 rg 0 0 40 40 re f Q 0 0 1 rg 0 35 40 5 re f",
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let texts = list
            .objects()
            .iter()
            .filter_map(|o| {
                if let PageObject::Text(t) = o {
                    Some(t)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(texts.len(), 2);
        assert!(
            texts
                .iter()
                .all(|t| t.has_interpreted_clipping() && t.state.clip_path.is_empty())
        );
        let paths = list
            .objects()
            .iter()
            .filter_map(|o| {
                if let PageObject::Path(p) = o {
                    Some(p)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            paths[0].graphic_state().clip_path.elements()[0]
                .path
                .subpaths()
                .len(),
            2
        );
        assert!(paths[1].graphic_state().clip_path.is_empty());
        let mut render = SkiaRender::new(72., 72.).unwrap();
        list.replay(&mut render).unwrap();
        let data = pixels(&mut render);
        for (x, y, color) in [
            (7, 30, [255, 0, 0, 255]),
            (27, 30, [255, 0, 0, 255]),
            (20, 30, [255, 255, 255, 255]),
            (20, 2, [0, 0, 255, 255]),
        ] {
            assert_eq!(&data[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4], color);
        }
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()
                .len(),
            2
        );
    }
}
#[test]
fn zero_and_signed_sizes_clip_to_actual_outline_not_glyph_bounds() {
    for (size, origin, inside) in [(0, "5 5", false), (-20, "35 25", true)] {
        let doc = document(&format!(
            "BT /FONT {size} Tf 7 Tr 1 0 0 1 {origin} Tm (A) Tj ET 1 0 0 rg 0 0 40 40 re f"
        ));
        let mut render = SkiaRender::new(72., 72.).unwrap();
        doc.get_page(0).unwrap().display(&mut render).unwrap();
        let data = pixels(&mut render);
        let pixel = &data[(20 * 40 + 30) * 4..(20 * 40 + 30) * 4 + 4];
        assert_eq!(
            pixel,
            if inside {
                [255, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        );
        if !inside {
            assert!(
                data.as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [255, 255, 255, 255])
            );
        }
    }
}

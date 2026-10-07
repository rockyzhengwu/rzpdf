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
    fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }
    fn do_image(&mut self, _: ImageObject) -> PdfResult<()> {
        Ok(())
    }
    fn show_text(&mut self, _: TextObject) -> PdfResult<()> {
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

use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

fn parse(value: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(value.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}

fn pattern(
    doc: &PdfDocument<Cursor<Vec<u8>>>,
    extras: &str,
    content: &[u8],
) -> pdf::objects::object_id::ObjectId {
    let dict = parse(&format!("<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] /XStep 10 /YStep 10 /Resources << >> {extras} >>")).into_dict().unwrap();
    doc.try_insert_object(PdfObject::PdfStream(
        PdfStream::from_decoded_data(dict, content.to_vec()).unwrap(),
    ))
    .unwrap()
}

fn document() -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new_with_options(ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    })
    .unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
        .unwrap();
    doc
}

#[test]
fn coloured_pattern_selection_and_lattice_survive_strict_save() {
    let doc = document();
    let id = pattern(&doc, "", b"1 0 0 rg 0 0 4 4 re f");
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/Pattern cs /{name} scn q 1 0 0 1 3 5 cm 0 0 30 30 re f Q").into_bytes(),
    )
    .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!()
        };
        let pdf::color::colorspace::ColorSpace::Pattern(space) =
            &path.graphic_state().color_state.fill_color_space
        else {
            panic!()
        };
        let paint = space.paint().unwrap();
        assert!(!paint.list.objects().is_empty());
        let PageObject::Path(cell) = &paint.list.objects()[0] else {
            panic!()
        };
        assert!(cell.source().is_none());
        // Selection before cm: the pattern lattice must not inherit translation 3,5.
        assert_eq!(cell.matrix().e() % 10., 0.);
        assert_eq!((40. - cell.matrix().f()) % 10., 0.);
    }
}

#[test]
fn malformed_pattern_registration_does_not_mutate_page_or_revision() {
    let doc = document();
    for extras in [
        "/PaintType 0",
        "/TilingType 4",
        "/XStep 0",
        "/YStep 0",
        "/Resources null",
        "/Matrix [0 0 0 0 0 0]",
    ] {
        let id = pattern(&doc, extras, b"");
        let revision = doc.revision();
        let before = doc.save_to_bytes().unwrap();
        assert!(
            doc.resources()
                .register_existing(0, ResourceKind::Pattern, id)
                .is_err(),
            "{extras}"
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
}

#[test]
fn uncoloured_pattern_uses_tint_and_rejects_colour_in_cell() {
    for cell in [b"0 0 4 4 re f".as_slice(), b"1 0 0 rg 0 0 4 4 re f"] {
        let doc = document();
        let id = pattern(&doc, "/PaintType 2", cell);
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Pattern, id)
            .unwrap();
        let cs = doc
            .resources()
            .register_color_space(0, parse("[/Pattern /DeviceRGB]"))
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("/{cs} cs 0 0.5 1 /{name} scn 0 0 40 40 re f").into_bytes(),
        )
        .unwrap();
        let result = doc.get_page(0).unwrap().display_list();
        if cell.starts_with(b"1") {
            assert!(matches!(result, Err(PdfError::ColorError(_))));
        } else {
            let list = result.unwrap();
            let PageObject::Path(path) = &list.objects()[0] else {
                panic!()
            };
            let pdf::color::colorspace::ColorSpace::Pattern(space) =
                &path.graphic_state().color_state.fill_color_space
            else {
                panic!()
            };
            let PageObject::Path(cell) = &space.paint().unwrap().list.objects()[0] else {
                panic!()
            };
            assert_eq!(
                cell.graphic_state().color_state.fill_color_value.values(),
                [0., 0.5, 1.]
            );
        }
    }
}

#[test]
fn compact_cell_operation_budget_and_retry_are_independent() {
    let doc = document();
    let id = pattern(&doc, "", b"0 0 4 4 re f");
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/Pattern cs /{name} scn 0 0 40 40 re f").into_bytes(),
    )
    .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let limited = PdfDocument::from_bytes_with_options(
        bytes,
        ParseOptions {
            mode: ParseMode::Strict,
            limits: Limits {
                max_content_operations: 3,
                ..Limits::default()
            },
        },
    )
    .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            limited.get_page(0).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    assert!(doc.get_page(0).unwrap().display_list().is_ok());
}

#[test]
fn pattern_base_shape_and_tint_count_are_validated() {
    let doc = document();
    for cs in ["[/Pattern /DeviceRGB /DeviceGray]", "[/Pattern /Pattern]"] {
        assert!(doc.resources().register_color_space(0, parse(cs)).is_err());
    }
    let id = pattern(&doc, "/PaintType 2", b"0 0 4 4 re f");
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, id)
        .unwrap();
    let cs = doc
        .resources()
        .register_color_space(0, parse("[/Pattern /DeviceRGB]"))
        .unwrap();
    for content in [
        format!("/{cs} cs 1 /{name} scn"),
        format!("/Pattern cs /{name} scn"),
    ] {
        doc.replace_page_contents(0, content.into_bytes()).unwrap();
        assert!(doc.get_page(0).unwrap().display_list().is_err());
    }
}

#[test]
fn shading_pattern_has_independent_matrix_background_and_state() {
    let doc = document();
    let id=doc.try_insert_object(parse("<< /Type /Pattern /PatternType 2 /Matrix [2 0 0 1 3 4] /ExtGState << /ca 0.5 /BM /Multiply >> /Shading << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 10 0] /Background [0.25] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >> >>")).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/Pattern cs /{name} scn q 1 0 0 1 7 8 cm 0 0 20 20 re f Q").into_bytes(),
    )
    .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!()
        };
        let pdf::color::colorspace::ColorSpace::Pattern(space) =
            &path.graphic_state().color_state.fill_color_space
        else {
            panic!()
        };
        let PageObject::Shading(shade) = &space.paint().unwrap().list.objects()[0] else {
            panic!()
        };
        assert!(shade.uses_pattern_background());
        assert_eq!(
            (shade.matrix.a(), shade.matrix.e(), shade.matrix.f()),
            (2., 3., 36.)
        );
        assert_eq!(shade.graphic_state.general_state.fill_alpha, 0.5);
        assert_eq!(
            shade.graphic_state.general_state.blend_mode,
            pdf::page::general_state::BlendMode::Multiply
        );
        assert_eq!(path.graphic_state().general_state.fill_alpha, 1.);
    }
}

#[test]
fn form_local_pattern_scope_and_default_matrix_do_not_use_body_cm() {
    let doc = document();
    let pid = pattern(&doc, "", b"0 0 1 rg 0 0 4 4 re f");
    let page_name = doc
        .resources()
        .register_existing(0, ResourceKind::Pattern, pid)
        .unwrap();
    let form_dict=parse(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Matrix [1 0 0 1 7 8] /Resources << /Pattern << /Local {} 0 R >> >> >>",pid.obj_num())).into_dict().unwrap();
    let fid = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                form_dict,
                b"1 0 0 1 3 5 cm /Pattern cs /Local scn 0 0 10 10 re f".to_vec(),
            )
            .unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, fid)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/{name} Do /Pattern cs /{page_name} scn 0 0 5 5 re f").into_bytes(),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!()
    };
    let pdf::color::colorspace::ColorSpace::Pattern(space) =
        &path.graphic_state().color_state.fill_color_space
    else {
        panic!()
    };
    let PageObject::Path(cell) = &space.paint().unwrap().list.objects()[0] else {
        panic!()
    };
    // Compact cells use local coordinates; the separate placement retains
    // the Form default transform and excludes the body's later cm (3,5).
    assert_eq!((cell.matrix().e(), cell.matrix().f()), (0., 0.));
    let placement = space.paint().unwrap().tile_matrix.as_ref().unwrap();
    assert_eq!((placement.e(), placement.f()), (7., 32.));
    // Empty local Pattern table cannot borrow the page Pattern resource.
    let bad_dict = parse("<< /Subtype /Form /BBox [0 0 20 20] /Resources << >> >>")
        .into_dict()
        .unwrap();
    doc.update_object(
        fid,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                bad_dict,
                format!("/Pattern cs /{page_name} scn").into_bytes(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert!(doc.get_page(0).unwrap().display_list().is_err());
}

#[test]
fn importing_pattern_copies_graph_and_binds_target_page() {
    let source = document();
    let id = pattern(&source, "", b"1 0 0 rg 0 0 4 4 re f");
    let target = document();
    let handle = source.object_handle(id).unwrap();
    let imported = target
        .import_resource_from(&source, &handle, ResourceKind::Pattern)
        .unwrap();
    let name = target
        .resources()
        .register_handle(0, ResourceKind::Pattern, &imported)
        .unwrap();
    target
        .replace_page_contents(
            0,
            format!("/Pattern cs /{name} scn 0 0 40 40 re f").into_bytes(),
        )
        .unwrap();
    assert!(target.get_page(0).unwrap().display_list().is_ok());
}

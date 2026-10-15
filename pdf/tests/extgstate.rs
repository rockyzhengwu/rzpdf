use pdf::{
    document::owned::Document,
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict},
    page::{
        graph_state::{LineCap, LineJoin},
        page_builder::PageBuilder,
        page_object::PageObject,
    },
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn document(state: &str, content: &[u8]) -> Document {
    let doc = Document::new().unwrap();
    let state =
        SyntaxParser::new(StreamReader::try_new(Cursor::new(state.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap();
    let mut states = PdfDict::default();
    states.insert("G".into(), state);
    let mut resources = PdfDict::default();
    resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
    doc.add_page_with_builder(
        0,
        PageBuilder::new()
            .resources(resources)
            .content(content.to_vec()),
    )
    .unwrap();
    doc
}

#[test]
fn extgstate_line_parameters_apply_and_restore_through_q() {
    let doc = document(
        "<< /Type /ExtGState /LW 3 /LC 2 /LJ 1 /ML 8 /FL 15 /D [[4 2] 1] >>",
        b"q /G gs 0 0 10 10 re S Q 20 0 10 10 re S",
    );
    for doc in [
        doc,
        Document::from_bytes_with_mode(
            document(
                "<< /Type /ExtGState /LW 3 /LC 2 /LJ 1 /ML 8 /FL 15 /D [[4 2] 1] >>",
                b"q /G gs 0 0 10 10 re S Q 20 0 10 10 re S",
            )
            .save_to_bytes()
            .unwrap(),
            ParseMode::Strict,
        )
        .unwrap(),
    ] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        assert_eq!(list.objects().len(), 2);
        let PageObject::Path(first) = &list.objects()[0] else {
            panic!("expected path");
        };
        let state = first.graphic_state();
        assert_eq!(state.graph_state.line_width, 3.0);
        assert!(matches!(state.graph_state.line_cap, LineCap::Square));
        assert!(matches!(state.graph_state.line_join, LineJoin::Round));
        assert_eq!(state.graph_state.miter_limit, 8.0);
        assert_eq!(state.graph_state.dash_array, vec![4.0, 2.0]);
        assert_eq!(state.graph_state.dash_phrase, 1.0);
        assert_eq!(state.general_state.flat_ness, 15.0);
        let PageObject::Path(second) = &list.objects()[1] else {
            panic!("expected path");
        };
        assert_eq!(second.graphic_state().graph_state.line_width, 1.0);
        assert!(second.graphic_state().graph_state.dash_array.is_empty());
    }
}

#[test]
fn unsupported_extgstate_is_diagnosed_without_partial_application() {
    let doc = document("<< /LW 99 /ca 0.5 /AIS true >>", b"/G gs 0 0 10 10 re S");
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    assert_eq!(list.diagnostics().len(), 1);
    assert_eq!(list.diagnostics()[0].operator, "gs");
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("expected path");
    };
    assert_eq!(path.graphic_state().graph_state.line_width, 1.0);
    let strict =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(matches!(
        strict.get_page(0).unwrap().display_list(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn invalid_extgstate_values_and_missing_resources_fail() {
    for state in [
        "<< /LW -1 >>",
        "<< /LC 1.5 >>",
        "<< /LJ 3 >>",
        "<< /ML 0.5 >>",
        "<< /FL 101 >>",
        "<< /D [[0 0] 0] >>",
        "<< /D [[1] -1] >>",
        "<< /D [1 0] >>",
        "<< /Font [null 12] >>",
        "<< /Font [] >>",
        "<< /Type /Wrong >>",
        "<< /ca -0.1 >>",
        "<< /CA 1.1 >>",
        "<< /ca /Bad >>",
    ] {
        assert!(
            document(state, b"/G gs")
                .get_page(0)
                .unwrap()
                .display_list()
                .is_err(),
            "{state}"
        );
    }
    assert!(
        document("<< >>", b"/Missing gs")
            .get_page(0)
            .unwrap()
            .display_list()
            .is_err()
    );
    assert!(
        document("<< >>", b"/G gs")
            .get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[test]
fn alpha_constants_default_to_one_and_restore_with_q() {
    let doc = document(
        "<< /ca 0.5 /CA 0.25 >>",
        b"q /G gs 0 0 10 10 re B Q 20 0 10 10 re B",
    );
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [doc, reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        let PageObject::Path(first) = &list.objects()[0] else {
            panic!("expected path");
        };
        let PageObject::Path(second) = &list.objects()[1] else {
            panic!("expected path");
        };
        assert_eq!(first.graphic_state().general_state.fill_alpha, 0.5);
        assert_eq!(first.graphic_state().general_state.stroke_alpha, 0.25);
        assert_eq!(second.graphic_state().general_state.fill_alpha, 1.0);
        assert_eq!(second.graphic_state().general_state.stroke_alpha, 1.0);
    }
}

#[test]
fn blend_modes_parse_preferences_restore_and_strictly_round_trip() {
    use pdf::page::general_state::BlendMode;
    for (value, expected) in [
        ("/Multiply", BlendMode::Multiply),
        ("/Compatible", BlendMode::Normal),
        ("[/Unknown /Screen /Multiply]", BlendMode::Screen),
        ("/Luminosity", BlendMode::Luminosity),
        ("[/Unknown /Other]", BlendMode::Normal),
        ("[]", BlendMode::Normal),
    ] {
        let doc = document(
            &format!("<< /BM {value} >>"),
            b"q /G gs 0 0 10 10 re f Q 20 0 10 10 re f",
        );
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [doc, reopened] {
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            assert!(list.diagnostics().is_empty());
            let PageObject::Path(first) = &list.objects()[0] else {
                panic!("expected path");
            };
            let PageObject::Path(second) = &list.objects()[1] else {
                panic!("expected path");
            };
            assert_eq!(first.graphic_state().general_state.blend_mode, expected);
            assert_eq!(
                second.graphic_state().general_state.blend_mode,
                BlendMode::Normal
            );
        }
    }
    for value in ["1", "[/Normal 1]", "[null]"] {
        assert!(
            document(&format!("<< /BM {value} >>"), b"/G gs")
                .get_page(0)
                .unwrap()
                .display_list()
                .is_err()
        );
    }
    for value in ["/Unknown"] {
        let doc = document(
            &format!("<< /LW 99 /BM {value} >>"),
            b"/G gs 0 0 10 10 re S",
        );
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.diagnostics().len(), 1);
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("expected path");
        };
        assert_eq!(path.graphic_state().graph_state.line_width, 1.0);
        let strict =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        assert!(matches!(
            strict.get_page(0).unwrap().display_list(),
            Err(PdfError::UnsupportedFeature(_))
        ));
    }
}

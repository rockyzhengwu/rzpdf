use pdf::{
    document::{PdfDocument, resource_manager::StandardFont},
    page::diagnostic::DiagnosticKind,
    parser::syntax::ParseMode,
};

#[test]
fn plain_marked_content_is_balanced_without_hiding_text() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    for opening in ["/Span BMC", "/Span << /MCID 7 >> BDC"] {
        doc.replace_page_contents(
            0,
            format!("{opening} BT /{font} 12 Tf 10 10 Td (ABC) Tj ET EMC").into_bytes(),
        )
        .unwrap();
        let bytes = doc.save_to_bytes().unwrap();
        let strict = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        let report = strict
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report
                .characters
                .into_iter()
                .map(|item| item.unicode.unwrap())
                .collect::<String>(),
            "ABC"
        );
    }

    for content in [
        b"EMC".as_slice(),
        b"/Span BMC".as_slice(),
        b"(Span) BMC EMC".as_slice(),
    ] {
        doc.replace_page_contents(0, content.to_vec()).unwrap();
        assert!(doc.get_page(0).unwrap().display_list().is_err());
    }
}

#[test]
fn property_marked_content_stays_diagnostic() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"/Span << /ActualText (replacement) >> BDC EMC".to_vec())
        .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let compat = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Compatible).unwrap();
    let report = compat
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].operator, "BDC");
    assert_eq!(report.diagnostics[0].kind, DiagnosticKind::UnsupportedOperator);
    let strict = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(strict.get_page(0).unwrap().display_list().is_err());
}

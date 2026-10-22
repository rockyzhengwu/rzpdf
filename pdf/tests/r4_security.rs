use pdf::{
    document::metadata::InfoTextField,
    document::{Document, save_options::SaveOptions},
    error::PdfError,
    security::Authentication,
};
const EMPTY: &[u8] = include_bytes!("fixtures/r4/empty-user.pdf");
const PASSWORD: &[u8] = include_bytes!("fixtures/r4/password-user.pdf");
#[test]
fn r4_aes128_authentication_permissions_streams_and_readonly_save() {
    let empty = Document::from_bytes(EMPTY.to_vec()).unwrap();
    assert_eq!(empty.page_count(), 1);
    assert_eq!(
        empty.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("R4 synthetic title")
    );
    let mut output = vec![42];
    assert!(matches!(
        empty.save_to_writer(&mut output),
        Err(PdfError::UnsupportedSecurity(_))
    ));
    assert_eq!(output, vec![42]);
    let report = empty.security_report().unwrap();
    assert_eq!(report.authentication, Authentication::User);
    assert!(!report.permissions.modify);
    assert!(!report.permissions.copy);
    assert!(
        empty
            .get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .diagnostics()
            .is_empty()
    );
    assert!(matches!(
        empty.save_to_bytes(),
        Err(PdfError::UnsupportedSecurity(_))
    ));
    assert!(matches!(
        Document::from_bytes(PASSWORD.to_vec()),
        Err(PdfError::PasswordRequired)
    ));
    assert!(matches!(
        Document::from_bytes_with_password(PASSWORD.to_vec(), "wrong"),
        Err(PdfError::InvalidPassword)
    ));
    for (password, role) in [
        ("synthetic-user", Authentication::User),
        ("synthetic-owner", Authentication::Owner),
    ] {
        let doc = Document::from_bytes_with_password(PASSWORD.to_vec(), password).unwrap();
        assert_eq!(doc.security_report().unwrap().authentication, role);
        assert_eq!(
            doc.info_text(InfoTextField::Title).unwrap().as_deref(),
            Some("R4 synthetic title")
        );
        assert!(
            doc.get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .diagnostics()
                .is_empty()
        );
        assert!(matches!(
            doc.save_to_bytes_with_options(SaveOptions::default()),
            Err(PdfError::UnsupportedSecurity(_))
        ));
    }
}

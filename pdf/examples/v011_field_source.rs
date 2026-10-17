//! Synthetic field/Unicode appearance source; no production signing identity.
use pdf::{
    document::{
        Document,
        acroform::{FormFieldDefinition, FormFieldKind, FormValue, FormWidgetSpec},
        embedded_font::TrueTypeEmbeddingOptions,
        save_options::PdfVersion,
    },
    parser::syntax::ParseMode,
};
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("NEW_OUTPUT required")?;
    let doc = Document::new_with_version(PdfVersion::V2_0)?;
    doc.add_page(0)?;
    doc.add_page(1)?;
    let font = doc.resources().embed_truetype_font(
        0,
        include_bytes!("../tests/fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
        "你好世界Hello PDF，！",
        TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Thin".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
    )?;
    doc.add_form_field(
        "Data.Editable",
        &FormFieldDefinition::new(
            FormFieldKind::Text { multiline: false },
            FormValue::Text("Original".into()),
        ),
        &[FormWidgetSpec::new(0, [20., 100., 300., 140.])],
    )?;
    doc.add_form_field(
        "Data.Check",
        &FormFieldDefinition::new(FormFieldKind::Checkbox, FormValue::Checkbox(false)),
        &[FormWidgetSpec::new(0, [320., 100., 340., 120.])],
    )?;
    doc.add_signature_field(
        "Sign.First",
        &[
            FormWidgetSpec::new(0, [20., 180., 300., 230.]),
            FormWidgetSpec::new(1, [20., 180., 300., 230.]),
        ],
    )?;
    doc.add_signature_field(
        "Sign.Second",
        &[FormWidgetSpec::new(0, [20., 260., 300., 310.])],
    )?;
    let bytes = doc.save_to_bytes()?;
    let check = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    check.form_field_tree()?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    target.write_all(&bytes)?;
    target.sync_all()?;
    println!("font={}", font.object_handle().object_id().obj_num());
    Ok(())
}

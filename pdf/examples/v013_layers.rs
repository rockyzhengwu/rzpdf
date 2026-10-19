//! Author layers on an independently produced two-page seed; new output only.
use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        optional_content_edit::*,
        save_options::{IncrementalSaveLayout, SaveLayout, SaveOptions, StreamCompression},
    },
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
    security::{EncryptionOptions, SaveSecurity},
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn output(path: &Path, bytes: &[u8]) -> PdfResult<()> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
fn main() -> PdfResult<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() != 2 {
        return Err(PdfError::DocumentError("seed.pdf NEW_DIR".into()));
    }
    let d = Document::from_bytes_with_mode(std::fs::read(&a[0])?, ParseMode::Strict)?;
    let b = OperationBudget::new(128 * 1024 * 1024);
    let red = d.create_layer("红色 Red", LayerIntent::View, &b)?;
    let blue = d.create_layer("蓝色 Blue", LayerIntent::View, &b)?;
    let target = d.layer_configuration_target(None)?;
    d.write_layer_configuration(
        Some(&target),
        &LayerConfiguration {
            name: Some("Default".into()),
            base: LayerBaseState::On,
            on: vec![red.clone()],
            off: vec![blue.clone()],
        },
        &b,
    )?;
    d.write_layer_configuration(
        None,
        &LayerConfiguration {
            name: Some("Swap".into()),
            base: LayerBaseState::Off,
            on: vec![blue.clone()],
            off: vec![red.clone()],
        },
        &b,
    )?;
    d.write_layer_configuration(
        None,
        &LayerConfiguration {
            name: Some("Both".into()),
            base: LayerBaseState::On,
            on: vec![],
            off: vec![],
        },
        &b,
    )?;
    let member = d.create_layer_membership(
        &[red.clone(), blue.clone()],
        LayerPolicy::AllOff,
        Some(&LayerExpression::And(vec![
            LayerExpression::Group(red.clone()),
            LayerExpression::Not(Box::new(LayerExpression::Group(blue.clone()))),
        ])),
        &b,
    )?;
    for page in 0..2 {
        d.append_layer_content(page, &red, b"1 0 0 rg 10 100 40 40 re f", &b)?;
        d.append_layer_content(page, &blue, b"0 0 1 rg 70 100 40 40 re f", &b)?;
        d.append_layer_content(page, &member, b"1 0.5 0 rg 10 60 40 20 re f", &b)?;
    }
    // Shared Form, deliberately bound on page 0 only.
    let mut parser = pdf::parser::syntax::SyntaxParser::new(
        pdf::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources <<>> >>".to_vec(),
        ))?,
    );
    let dict = parser.get_object()?.into_dict().unwrap();
    let id = d.try_insert_object(pdf::objects::PdfObject::PdfStream(
        pdf::objects::pdf_stream::PdfStream::from_decoded_data(
            dict,
            b"0 1 0 rg 130 100 30 30 re f".to_vec(),
        )?,
    ))?;
    let h = d.object_handle(id)?;
    for page in 0..2 {
        let n = d.resources().register_handle(
            page,
            pdf::document::resource_manager::ResourceKind::Form,
            &h,
        )?;
        d.append_page_contents(page, format!("/{n} Do").into_bytes())?;
    }
    let p = d.get_page(0)?.content_program()?;
    d.bind_layer_xobject_call(0, &p, p.operations().len() - 1, &red, &b)?;
    let out = Path::new(&a[1]);
    std::fs::create_dir(out)?;
    for (label, l) in [
        ("classic", SaveLayout::Classic),
        ("xref", SaveLayout::XrefStream),
        ("objstm", SaveLayout::XrefStreamWithObjectStreams),
    ] {
        for (encoding, c) in [
            ("preserve", StreamCompression::Preserve),
            ("flate", StreamCompression::FlateUnencoded),
        ] {
            let options = SaveOptions {
                version: Some(pdf::document::save_options::PdfVersion::V2_0),
                compression: c,
                ..Default::default()
            };
            let bytes = d.save_to_bytes_with_layout(options, l)?;
            output(&out.join(format!("{label}-{encoding}.pdf")), &bytes)?;
            let reopened = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
            let group = reopened.optional_content()?.unwrap().groups[0]
                .handle
                .clone();
            reopened.rename_layer(&group, "Renamed Red", &b)?;
            for (suffix, append) in [
                ("table", IncrementalSaveLayout::Classic),
                ("stream", IncrementalSaveLayout::XrefStream),
            ] {
                let incremental =
                    reopened.save_incremental_to_bytes_with_layout(Default::default(), append)?;
                assert!(incremental.starts_with(&bytes));
                output(
                    &out.join(format!("{label}-{encoding}-{suffix}.pdf")),
                    &incremental,
                )?;
            }
            let settings = EncryptionOptions::new("user", "owner")?;
            let encrypted =
                d.save_to_bytes_with_layout_and_security(options, l, SaveSecurity::Set(&settings))?;
            output(&out.join(format!("{label}-{encoding}-r6.pdf")), &encrypted)?;
        }
    }
    assert_eq!(b.live_bytes(), 0);
    println!(
        "groups=2 configurations=2 tracked_peak={} live=0",
        b.peak_bytes()
    );
    Ok(())
}

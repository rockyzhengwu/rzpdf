//! Project-authored interaction fixtures; output remains ignored and inputs unchanged.
use pdf::{
    document::{
        Document,
        acroform::{
            ChoiceOption, FormFieldDefinition as Def, FormFieldKind as Kind, FormTextStyle,
            FormValue as Val, FormWidgetSpec as Widget,
        },
        annotations::{Annotation, AnnotationKind},
        embedded_font::TrueTypeEmbeddingOptions,
    },
    page::page_builder::PageBuilder,
    security::{EncryptionOptions, SaveSecurity},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&root)?;
    let d = Document::new()?;
    d.add_page_with_builder(0, PageBuilder::new().size(300., 300.))?;
    d.add_text_field(0, [20., 240., 180., 264.], "name", "Old")?;
    d.add_checkbox_field(0, [20., 205., 40., 225.], "accepted", false)?;
    let mut choice = Def::new(
        Kind::Choice {
            combo: false,
            editable: false,
            multiple: true,
        },
        Val::Choice(vec!["A".into()]),
    );
    choice.options = vec![
        ChoiceOption {
            export: "A".into(),
            display: "First".into(),
        },
        ChoiceOption {
            export: "B".into(),
            display: "Second".into(),
        },
    ];
    d.add_form_field(
        "selection",
        &choice,
        &[Widget::new(0, [60., 180., 180., 226.])],
    )?;
    let radio = Def::new(
        Kind::Radio {
            no_toggle_to_off: false,
            in_unison: false,
        },
        Val::Radio(Some("A".into())),
    );
    let mut a = Widget::new(0, [200., 240., 220., 260.]);
    a.on_state = Some("A".into());
    let mut b = Widget::new(0, [240., 240., 260., 260.]);
    b.on_state = Some("B".into());
    d.add_form_field("radio", &radio, &[a, b])?;
    let font = d.resources().embed_truetype_font(
        0,
        include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
        "你好世界Hello PDF，！",
        TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Thin".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
    )?;
    let mut unicode = Def::new(Kind::Text { multiline: false }, Val::Text("你好".into()));
    unicode.text_style = FormTextStyle {
        font: Some(font.object_handle().clone()),
        font_size: 14.,
        ..Default::default()
    };
    d.add_form_field(
        "unicode",
        &unicode,
        &[Widget::new(0, [20., 135., 180., 160.])],
    )?;
    let mut ann = Annotation::new(AnnotationKind::Square, [200., 180., 260., 220.]);
    ann.color = [1., 0., 0.];
    ann.stroke_width = 2.;
    ann.contents = "Initial note".into();
    d.add_annotation(0, &ann)?;
    std::fs::write(root.join("interaction.pdf"), d.save_to_bytes()?)?;
    let mut protection = EncryptionOptions::new("user", "owner")?;
    protection.permissions.modify = false;
    protection.permissions.annotate = false;
    protection.permissions.fill_forms = true;
    std::fs::write(
        root.join("interaction-protected.pdf"),
        d.save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&protection))?,
    )?;
    let source = std::fs::read(root.join("interaction.pdf"))?;
    let visibility = Document::from_bytes(source.clone())?;
    for field in visibility.form_field_records()? {
        let flags = match field.name.as_str() {
            "name" => 2,
            "accepted" => 32,
            "radio" => 64,
            _ => 4,
        };
        for widget in field.widgets {
            let mut dict = visibility
                .resolve_handle(&widget.handle)?
                .into_dict()
                .unwrap();
            dict.try_insert(
                "F".into(),
                pdf::objects::PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(
                    flags as f32,
                )),
            )?;
            visibility.update_object(
                widget.handle.object_id(),
                pdf::objects::PdfObject::PdfDict(dict),
            )?;
        }
    }
    use pdf::document::optional_content_edit::{LayerBaseState, LayerConfiguration, LayerIntent};
    let budget = pdf::budget::OperationBudget::new(64 * 1024 * 1024);
    let layer = visibility.create_layer("Hidden widget", LayerIntent::View, &budget)?;
    let target = visibility.layer_configuration_target(None)?;
    visibility.write_layer_configuration(
        Some(&target),
        &LayerConfiguration {
            name: None,
            base: LayerBaseState::On,
            on: vec![],
            off: vec![layer.clone()],
        },
        &budget,
    )?;
    let record = visibility
        .form_field_records()?
        .into_iter()
        .find(|f| f.name == "unicode")
        .unwrap();
    for widget in record.widgets {
        let mut dict = visibility
            .resolve_handle(&widget.handle)?
            .into_dict()
            .unwrap();
        dict.try_insert(
            "OC".into(),
            pdf::objects::PdfObject::PdfReference(
                pdf::objects::pdf_reference::PdfReference::from_id(layer.object_id()),
            ),
        )?;
        visibility.update_object(
            widget.handle.object_id(),
            pdf::objects::PdfObject::PdfDict(dict),
        )?;
    }
    let mut combo = Def::new(
        Kind::Choice {
            combo: true,
            editable: true,
            multiple: false,
        },
        Val::Choice(vec!["A".into()]),
    );
    combo.options = vec![ChoiceOption {
        export: "A".into(),
        display: "First".into(),
    }];
    visibility.add_form_field(
        "editable",
        &combo,
        &[Widget::new(0, [20., 80., 180., 110.])],
    )?;
    let unison = Def::new(
        Kind::Radio {
            no_toggle_to_off: true,
            in_unison: true,
        },
        Val::Radio(Some("A".into())),
    );
    let mut widgets = vec![
        Widget::new(0, [200., 80., 220., 100.]),
        Widget::new(0, [230., 80., 250., 100.]),
        Widget::new(0, [260., 80., 280., 100.]),
    ];
    for (widget, state) in widgets.iter_mut().zip(["A", "A", "B"]) {
        widget.on_state = Some(state.into());
    }
    visibility.add_form_field("unison", &unison, &widgets)?;
    std::fs::write(
        root.join("interaction-visibility.pdf"),
        visibility.save_to_bytes()?,
    )?;
    for rotation in [0, 90, 180, 270] {
        let geometry = Document::from_bytes(source.clone())?;
        geometry.set_page_rotation(0, rotation)?;
        geometry.set_page_user_unit(0, Some(1.5))?;
        geometry.set_page_crop_box(
            0,
            Some(pdf::geom::rectangle::Rectangle::new(10., 10., 290., 290.)),
        )?;
        std::fs::write(
            root.join(format!("interaction-rotation-{rotation}.pdf")),
            geometry.save_to_bytes()?,
        )?;
        if rotation == 90 {
            let source = geometry.save_to_bytes()?;
            for flags in [12, 20, 28] {
                let sample = Document::from_bytes(source.clone())?;
                let field = sample
                    .form_field_records()?
                    .into_iter()
                    .find(|f| f.name == "name")
                    .unwrap();
                for widget in field.widgets {
                    let mut dict = sample.resolve_handle(&widget.handle)?.into_dict().unwrap();
                    dict.try_insert(
                        "F".into(),
                        pdf::objects::PdfObject::PdfNumber(
                            pdf::objects::pdf_number::PdfNumber::new(flags as f32),
                        ),
                    )?;
                    sample.update_object(
                        widget.handle.object_id(),
                        pdf::objects::PdfObject::PdfDict(dict),
                    )?;
                }
                std::fs::write(
                    root.join(format!("interaction-flags-{flags}.pdf")),
                    sample.save_to_bytes()?,
                )?;
            }
        }
    }
    let records = d.form_field_records()?;
    let unknown = records.iter().find(|f| f.name == "name").unwrap();
    let mut dict = d.resolve_handle(&unknown.handle)?.into_dict().unwrap();
    dict.try_insert(
        "Ff".into(),
        pdf::objects::PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(
            (1u32 << 13) as f32,
        )),
    )?;
    d.update_object(
        unknown.handle.object_id(),
        pdf::objects::PdfObject::PdfDict(dict),
    )?;
    std::fs::write(root.join("interaction-mixed.pdf"), d.save_to_bytes()?)?;
    println!("interaction fixtures generated");
    Ok(())
}

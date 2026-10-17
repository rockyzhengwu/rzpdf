//! A reproducible Stage C corpus; no downloaded fonts or external actions.
use pdf::{
    document::{
        PdfDocument,
        acroform::{
            ChoiceOption, FormFieldDefinition as Definition, FormFieldKind as Kind, FormTextStyle,
            FormValue as Value, FormWidgetSpec as Widget,
        },
        annotations::TextAlignment,
        embedded_font::TrueTypeEmbeddingOptions,
    },
    page::page_builder::PageBuilder,
    parser::syntax::ParseMode,
};

fn widget(page: u32, rect: [f32; 4]) -> Widget {
    Widget::new(page, rect)
}
fn button(page: u32, x: f32, y: f32, state: &str) -> Widget {
    Widget {
        page_index: page,
        rect: [x, y, x + 24., y + 24.],
        on_state: Some(state.into()),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(2..=3).contains(&args.len()) {
        return Err("usage: v09_form_types NEW_output.pdf [independent_input.pdf]".into());
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("output already exists".into());
    }
    let doc = if let Some(input) = args.get(2) {
        let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
        doc.set_form_value("external.radio", Value::Radio(Some("Blue".into())))?;
        doc.set_form_value("external.check", Value::Checkbox(true))?;
        doc.set_form_value("external.list", Value::ChoiceIndices(vec![0, 2]))?;
        doc.set_form_value("external.combo", Value::Choice(vec!["export-b".into()]))?;
        doc.set_form_value("external.text", Value::Text("Updated\nsecond line".into()))?;
        let push = doc
            .form_field_tree()?
            .into_iter()
            .find(|f| f.name == "external.push")
            .ok_or("missing push")?;
        doc.set_pushbutton_caption(&push.handle, "Updated button")?;
        doc
    } else {
        let doc = PdfDocument::new()?;
        for page in 0..3 {
            doc.add_page_with_builder(page, PageBuilder::new().size(520., 520.))?;
            doc.replace_page_contents(page, b"q Q".to_vec())?;
        }
        doc.add_form_field(
            "controls.check",
            &Definition::new(Kind::Checkbox, Value::Checkbox(true)),
            &[
                button(0, 30., 440., "Approved"),
                button(2, 30., 440., "Approved"),
            ],
        )?;
        let radio = Definition::new(
            Kind::Radio {
                no_toggle_to_off: true,
                in_unison: false,
            },
            Value::Radio(Some("Blue".into())),
        );
        doc.add_form_field(
            "controls.radio",
            &radio,
            &[
                button(0, 90., 440., "Red"),
                button(0, 150., 440., "Blue"),
                button(2, 90., 440., "Green"),
            ],
        )?;
        let mut list = Definition::new(
            Kind::Choice {
                combo: false,
                editable: false,
                multiple: true,
            },
            Value::ChoiceIndices(vec![0, 2]),
        );
        list.options = vec![
            ChoiceOption {
                export: "a".into(),
                display: "Alpha".into(),
            },
            ChoiceOption {
                export: "b".into(),
                display: "Beta".into(),
            },
            ChoiceOption {
                export: "c".into(),
                display: "Gamma".into(),
            },
        ];
        doc.add_form_field(
            "controls.list",
            &list,
            &[
                widget(0, [30., 300., 220., 400.]),
                widget(2, [30., 300., 220., 400.]),
            ],
        )?;
        list.kind = Kind::Choice {
            combo: true,
            editable: true,
            multiple: false,
        };
        list.value = Value::Choice(vec!["Custom option".into()]);
        doc.add_form_field(
            "controls.combo",
            &list,
            &[widget(0, [260., 350., 480., 390.])],
        )?;
        let mut push = Definition::new(Kind::PushButton, Value::PushButton);
        push.caption = "Push button".into();
        push.text_style.alignment = TextAlignment::Center;
        doc.add_form_field(
            "controls.push",
            &push,
            &[widget(0, [260., 280., 480., 320.])],
        )?;
        let mut text = Definition::new(
            Kind::Text { multiline: true },
            Value::Text("First line\nSecond line".into()),
        );
        doc.add_form_field(
            "controls.text",
            &text,
            &[widget(0, [30., 160., 480., 240.])],
        )?;
        let font = doc.resources().embed_truetype_font(
            1,
            include_bytes!("../tests/fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
            "你好世界Hello PDF，！",
            TrueTypeEmbeddingOptions {
                postscript_name: "NotoSansSC-Thin".into(),
                cap_height: 733.,
                stem_v: 80.,
                serif: false,
            },
        )?;
        for (row, alignment) in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ]
        .into_iter()
        .enumerate()
        {
            text.kind = Kind::Text { multiline: false };
            text.value = Value::Text("你好，世界！".into());
            text.text_style = FormTextStyle {
                font: Some(font.object_handle().clone()),
                font_size: 20.,
                alignment,
            };
            let top = 460. - row as f32 * 60.;
            doc.add_form_field(
                &format!("unicode.align{row}"),
                &text,
                &[widget(1, [30., top - 40., 480., top])],
            )?;
        }
        text.kind = Kind::Text { multiline: true };
        text.value = Value::Text("你好\n世界".into());
        text.text_style.alignment = TextAlignment::Center;
        doc.add_form_field(
            "unicode.multiline",
            &text,
            &[
                widget(1, [30., 200., 240., 300.]),
                widget(2, [260., 300., 480., 400.]),
            ],
        )?;
        list.kind = Kind::Choice {
            combo: false,
            editable: false,
            multiple: true,
        };
        list.value = Value::ChoiceIndices(vec![1]);
        list.text_style = text.text_style.clone();
        list.options = vec![
            ChoiceOption {
                export: "hello".into(),
                display: "你好".into(),
            },
            ChoiceOption {
                export: "world".into(),
                display: "世界".into(),
            },
        ];
        doc.add_form_field(
            "unicode.choice",
            &list,
            &[widget(1, [270., 200., 480., 300.])],
        )?;
        push.caption = "你好！".into();
        push.text_style = text.text_style.clone();
        doc.add_form_field("unicode.push", &push, &[widget(1, [30., 120., 240., 160.])])?;
        let mut exported = Definition::new(
            Kind::Radio {
                no_toggle_to_off: false,
                in_unison: false,
            },
            Value::Radio(Some("1".into())),
        );
        exported.button_exports = Some(vec!["你好".into(), "世界".into()]);
        doc.add_form_field(
            "unicode.exports",
            &exported,
            &[button(1, 300., 130., "0"), button(1, 360., 130., "1")],
        )?;
        let mut dup = Definition::new(
            Kind::Radio {
                no_toggle_to_off: false,
                in_unison: true,
            },
            Value::Radio(Some("Same".into())),
        );
        doc.add_form_field(
            "controls.unison",
            &dup,
            &[button(2, 260., 440., "Same"), button(2, 320., 440., "Same")],
        )?;
        dup.kind = Kind::Radio {
            no_toggle_to_off: false,
            in_unison: false,
        };
        dup.value = Value::Radio(None);
        let exclusive = doc.add_form_field(
            "controls.exclusive",
            &dup,
            &[button(2, 380., 440., "Same"), button(2, 440., 440., "Same")],
        )?;
        let last = doc
            .form_field_tree()?
            .into_iter()
            .find(|n| n.handle == exclusive)
            .unwrap()
            .widgets[1]
            .handle
            .clone();
        doc.set_radio_widget(&last)?;
        let bytes = doc.save_to_bytes()?;
        let doc = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict)?;
        // No in-memory registered-font token is needed after reopening.
        doc.set_form_value("unicode.multiline", Value::Text("世界\n你好".into()))?;
        doc
    };
    doc.form_field_details()?;
    let bytes = doc.save_to_bytes()?;
    PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?.form_field_details()?;
    std::fs::write(&args[1], bytes)?;
    println!("saved Stage C corpus: {}", args[1]);
    Ok(())
}

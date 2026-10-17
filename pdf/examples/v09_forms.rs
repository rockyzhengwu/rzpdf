//! Stage C field relationships. An optional independent input uses the same
//! person.address.city / person.accepted names, but may inherit field values.
use pdf::document::{PdfDocument, acroform::FormValue};
use pdf::parser::syntax::ParseMode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(2..=3).contains(&args.len()) {
        return Err("usage: v09_forms NEW_output.pdf [independent_input.pdf]".into());
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("output already exists".into());
    }
    let doc = if let Some(input) = args.get(2) {
        PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?
    } else {
        let doc = PdfDocument::new()?;
        for page in 0..2 {
            doc.add_page(page)?;
            doc.replace_page_contents(page, b"q Q".to_vec())?;
        }
        doc.add_text_field(0, [20., 40., 180., 70.], "person.address.city", "Shanghai")?;
        doc.add_checkbox_field(0, [20., 90., 44., 114.], "person.accepted", false)?;
        let fields = doc.form_field_tree()?;
        doc.add_form_widget(&fields[2].handle, 1, [25., 150., 225., 190.])?;
        doc.add_form_widget(&fields[3].handle, 1, [25., 220., 49., 244.])?;
        doc
    };
    let fields = doc.form_field_tree()?;
    let city = &fields
        .iter()
        .find(|f| f.name == "person.address.city")
        .ok_or("city missing")?
        .handle;
    let temporary = doc.add_form_widget(city, 0, [240., 40., 430., 70.])?;
    doc.remove_form_widget(&temporary)?;
    doc.set_form_value("person.address.city", FormValue::Text("Suzhou".into()))?;
    doc.set_form_value("person.accepted", FormValue::Checkbox(true))?;
    let root = &fields
        .iter()
        .find(|f| f.name == "person")
        .ok_or("root missing")?
        .handle;
    doc.rename_form_field(root, "customer")?;
    assert_eq!(
        doc.form_field_tree()?
            .iter()
            .filter(|f| !f.widgets.is_empty())
            .count(),
        2
    );
    assert_eq!(
        doc.form_field_tree()?
            .iter()
            .map(|f| f.widgets.len())
            .sum::<usize>(),
        4
    );
    let bytes = doc.save_to_bytes()?;
    let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    assert_eq!(
        reopened.form_fields()?[0].value,
        FormValue::Text("Suzhou".into())
    );
    std::fs::write(&args[1], bytes)?;
    println!("saved two fields / four widgets: {}", args[1]);
    Ok(())
}

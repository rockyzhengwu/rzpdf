//! Explicit incremental save. --demo-interactions targets the checked-in v04
//! ReportLab fixture only; it is not a generic form-filling CLI.
use pdf::{
    document::{Document, acroform::FormValue, metadata::InfoTextField},
    parser::syntax::ParseMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=4).contains(&args.len()) {
        return Err("usage: v05_incremental INPUT OUTPUT [TITLE] [--demo-interactions]".into());
    }
    let target = std::path::Path::new(&args[1]);
    if target.exists() {
        return Err("refusing to overwrite existing output".into());
    }
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("input exceeds example's 512 MiB ceiling".into());
    }
    let document = Document::from_bytes_with_mode(std::fs::read(&args[0])?, ParseMode::Strict)?;
    let title = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("Incremental update");
    if args.get(3).is_some_and(|arg| arg == "--remove-info") {
        document.remove_info_dict()?;
    } else {
        document.set_info_text(InfoTextField::Title, Some(title))?;
    }
    if args.get(3).is_some_and(|arg| arg == "--demo-interactions") {
        document.set_form_value("external_text", FormValue::Text("Incremental".into()))?;
        document.set_form_value("external_check", FormValue::Checkbox(false))?;
        document.rename_flat_bookmark(0, "Updated bookmark")?;
        document.set_named_destination("incremental", 1)?;
        document.add_named_link(0, [10.0, 10.0, 80.0, 30.0], "incremental")?;
        document.add_attachment("new.txt", b"payload".to_vec())?;
        document.add_square_annotation(0, [20.0, 60.0, 60.0, 100.0], [1.0, 0.0, 0.0])?;
    } else if let Some(id) = args
        .get(3)
        .and_then(|arg| arg.strip_prefix("--delete-object="))
    {
        let (number, generation) = id.split_once(',').ok_or("expected NUMBER,GENERATION")?;
        document.delete_object(pdf::objects::object_id::ObjectId::new(
            number.parse()?,
            generation.parse()?,
        ))?;
    } else if args.len() == 4 && args[3] != "--remove-info" {
        return Err("unknown fourth argument".into());
    }
    document.save_incremental_to_file(target)?;
    Ok(())
}

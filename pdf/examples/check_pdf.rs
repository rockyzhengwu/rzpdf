//! Single-file worker for scripts/check_pdf_corpus.py. Page numbers are one-based.
use pdf::{document::Document, parser::syntax::ParseMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: check_pdf INPUT.pdf strict|compatible".into());
    }
    let mode = match args[1].to_str() {
        Some("strict") => ParseMode::Strict,
        Some("compatible") => ParseMode::Compatible,
        _ => return Err("Invalid parse mode".into()),
    };
    let doc = Document::open_with_mode(&args[0], mode)?;
    let pages = doc.try_page_count()?;
    println!("pages={pages}");
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut missing = 0usize;
    let mut characters = 0usize;
    let mut missing_samples = std::collections::BTreeMap::new();
    for index in 0..pages {
        let result = doc
            .get_page(u32::try_from(index)?)
            .and_then(|mut page| page.extract_characters_paint_order_report());
        match result {
            Ok(report) => {
                characters += report.characters.len();
                for character in report.characters.iter().filter(|c| c.unicode.is_none()) {
                    let key = (
                        character.font_name.clone(),
                        character.charcode,
                        character.code_length,
                    );
                    if missing_samples.len() < 20 || missing_samples.contains_key(&key) {
                        *missing_samples.entry(key).or_insert(0usize) += 1;
                    }
                }
                missing += report
                    .characters
                    .iter()
                    .filter(|c| c.unicode.is_none())
                    .count();
                for diagnostic in report
                    .diagnostics
                    .iter()
                    .take(20usize.saturating_sub(warnings))
                {
                    eprintln!("page {}: {:?}", index + 1, diagnostic);
                }
                warnings += report.diagnostics.len();
            }
            Err(error) => {
                errors += 1;
                if errors <= 20 {
                    eprintln!("page {}: {error}", index + 1);
                }
            }
        }
    }
    let structure = doc.structure_diagnostics()?;
    warnings += structure.len();
    for diagnostic in structure.iter().take(20) {
        eprintln!("structure: {diagnostic:?}");
    }
    for ((font, code, length), count) in missing_samples {
        eprintln!("missing Unicode: font={font:?} code=0x{code:X} length={length} count={count}");
    }
    println!(
        "errors={errors}\nwarnings={warnings}\nmissing_unicode={missing}\ncharacters={characters}"
    );
    std::process::exit(if errors > 0 {
        1
    } else if warnings > 0 || missing > 0 {
        2
    } else {
        0
    });
}

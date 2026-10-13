//! Fill one supported text field in an existing PDF.
//! Usage: cargo run -p pdf --example v04_fill_text -- INPUT.pdf OUTPUT.pdf FIELD VALUE

use pdf::{
    document::{PdfDocument, acroform::FormValue},
    error::{PdfError, PdfResult},
};

fn main() -> PdfResult<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("input path required".into()))?;
    let output = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("output path required".into()))?;
    let field = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("field name required".into()))?;
    let value = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("value required".into()))?;
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::open(input)?;
    doc.set_form_value(&field, FormValue::Text(value))?;
    doc.save_to_file(output)
}

//! Fixed synthetic 1/10/100 MiB sources with one metadata-object addition.
//! OperationBudget peak is tracked temporary memory, NOT total process RSS.
use pdf::{
    document::{Document, metadata::InfoTextField},
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    parser::syntax::ParseMode,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for mib in [1usize, 10, 100] {
        let created = Document::new()?;
        created.add_page(0)?;
        let id = created.try_insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            vec![b'x'; mib * 1024 * 1024],
        )))?;
        let source = created.save_to_bytes()?;
        let source_bytes = source.len();
        drop(created);
        let document = Document::from_bytes_with_mode(source, ParseMode::Strict)?;
        document.set_info_text(InfoTextField::Title, Some("benchmark"))?;
        let budget = pdf::budget::OperationBudget::new(512 * 1024 * 1024);
        let start = Instant::now();
        let output = document.save_incremental_with_budget(Default::default(), &budget)?;
        let elapsed = start.elapsed();
        let tail_bytes = output.as_slice().len() - source_bytes;
        if tail_bytes > 4096 {
            return Err("single-object tail unexpectedly large".into());
        }
        println!(
            "{{\"payload_mib\":{mib},\"source_bytes\":{source_bytes},\"source_objects_including_zero\":{},\"changed_objects\":1,\"tail_bytes\":{tail_bytes},\"save_milliseconds\":{},\"tracked_peak_bytes\":{}}}",
            id.obj_num() + 1,
            elapsed.as_millis(),
            budget.peak_bytes()
        );
        drop(output);
        if budget.live_bytes() != 0 {
            return Err("operation reservation leak".into());
        }
    }
    Ok(())
}

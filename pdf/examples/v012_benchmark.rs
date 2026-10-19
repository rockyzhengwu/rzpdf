//! Fixed-scale structure baseline; tracked allocations are not process RSS.
use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        recovery::{RecoveredDocument, RecoveryLimits},
        save_options::{SaveLayout, SaveOptions, StreamCompression},
    },
    objects::{PdfObject, pdf_string::PdfString},
    parser::syntax::ParseMode,
};
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for count in [99usize, 100, 101, 1000, 10000] {
        let doc = Document::new()?;
        doc.add_page(0)?;
        for i in 0..count {
            doc.try_insert_object(PdfObject::PdfString(PdfString::new(
                format!("small-object-{i:08}").into_bytes(),
                false,
            )))?;
        }
        // Sparse numbering is deliberate, not a large dense object allocation.
        for _ in 0..1024 {
            let hole = doc.try_insert_object(PdfObject::PdfNull)?;
            doc.delete_object(hole)?;
        }
        doc.try_insert_object(PdfObject::PdfNull)?;
        for (layout, name) in [
            (SaveLayout::Classic, "classic"),
            (SaveLayout::XrefStream, "stream"),
            (SaveLayout::XrefStreamWithObjectStreams, "objects"),
        ] {
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let budget = OperationBudget::new(512 * 1024 * 1024);
                let start = Instant::now();
                let output = doc.save_with_layout_and_budget(
                    SaveOptions {
                        compression,
                        ..Default::default()
                    },
                    layout,
                    &budget,
                )?;
                println!(
                    "{{\"profile\":\"{name}\",\"compression\":\"{compression:?}\",\"small_objects\":{count},\"output_bytes\":{},\"microseconds\":{},\"tracked_peak_bytes\":{}}}",
                    output.as_slice().len(),
                    start.elapsed().as_micros(),
                    budget.peak_bytes()
                );
                let bytes = output.as_slice().to_vec();
                drop(output);
                assert_eq!(budget.live_bytes(), 0);
                let parsed = Document::from_bytes_with_mode(bytes, ParseMode::Strict)?;
                assert_eq!(parsed.try_page_count()?, 1);
            }
        }
        let source = doc.save_to_bytes()?;
        let size = source.len();
        let startxref = source
            .windows(10)
            .rposition(|w| w == b"startxref\n")
            .ok_or("startxref")?
            + 10;
        let end = startxref
            + source[startxref..]
                .iter()
                .position(|b| *b == b'\n')
                .ok_or("end")?;
        let mut broken = source.clone();
        broken.splice(startxref..end, b"0".iter().copied());
        let budget = OperationBudget::new(512 * 1024 * 1024);
        let start = Instant::now();
        let recovered = RecoveredDocument::from_bytes_with_budget(
            broken,
            Default::default(),
            RecoveryLimits::default(),
            &budget,
        )?;
        println!(
            "{{\"profile\":\"recovery\",\"small_objects\":{count},\"input_bytes\":{size},\"microseconds\":{},\"scan_steps\":{},\"tracked_peak_bytes\":{}}}",
            start.elapsed().as_micros(),
            recovered.scan_steps(),
            budget.peak_bytes()
        );
        drop(recovered);
        assert_eq!(budget.live_bytes(), 0);
        let mut current = source;
        for round in 0..3 {
            let parsed = Document::from_bytes_with_mode(current, ParseMode::Strict)?;
            parsed.set_info_text(
                pdf::document::metadata::InfoTextField::Title,
                Some(&format!("round-{round}")),
            )?;
            let budget = OperationBudget::new(512 * 1024 * 1024);
            let start = Instant::now();
            let bytes = parsed.save_incremental_with_layout_and_budget(
                Default::default(),
                pdf::document::save_options::IncrementalSaveLayout::XrefStream,
                &budget,
            )?;
            println!(
                "{{\"profile\":\"incremental\",\"small_objects\":{count},\"round\":{round},\"output_bytes\":{},\"microseconds\":{},\"tracked_peak_bytes\":{}}}",
                bytes.as_slice().len(),
                start.elapsed().as_micros(),
                budget.peak_bytes()
            );
            current = bytes.as_slice().to_vec();
            drop(bytes);
            assert_eq!(budget.live_bytes(), 0);
        }
    }
    Ok(())
}

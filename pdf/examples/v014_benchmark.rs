//! Fixed-scale debug measurements. Tracked reservations are not process RSS.
use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        associated_files::{AssociatedFileDefinition, AssociatedFileRelationship},
        save_options::{PdfVersion, SaveLayout},
    },
    parser::syntax::ParseMode,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<_> = [1, 100, 1000]
        .into_iter()
        .map(|n| ("entries", n, 8, 1))
        .chain(
            [0, 1024, 1024 * 1024]
                .into_iter()
                .map(|n| ("payload", 1, n, 1)),
        )
        .chain([1, 10, 100].into_iter().map(|n| ("owners", 1, 8, n)))
        .chain([("cumulative", 8, 64 * 1024, 1)])
        .collect();
    for (axis, count, size, owners) in cases {
        let d = Document::new_with_version(PdfVersion::V2_0)?;
        for i in 0..owners {
            d.add_page(i as u32)?;
        }
        let b = OperationBudget::new(256 * 1024 * 1024);
        let data = vec![b'x'; size];
        let start = std::time::Instant::now();
        for i in 0..count {
            let file = d.create_associated_file(
                &AssociatedFileDefinition::new(
                    format!("file-{i}.bin"),
                    "application/octet-stream",
                    AssociatedFileRelationship::Data,
                ),
                &data,
                &b,
            )?;
            if axis == "owners" {
                for page in 0..owners {
                    let owner = d.associated_file_owner(Some(page as u32), &b)?;
                    d.bind_associated_file(&owner, &file, &b)?;
                }
            } else {
                let owner = d.associated_file_owner(None, &b)?;
                d.bind_associated_file(&owner, &file, &b)?;
            }
        }
        let edit_us = start.elapsed().as_micros();
        let start = std::time::Instant::now();
        let target = d.associated_file_owner(if axis == "owners" { Some(0) } else { None }, &b)?;
        let list = d.associated_files(&target, &b)?;
        assert_eq!(list.files().len(), count);
        for i in 0..count {
            assert_eq!(d.associated_file_data(&list, i, &b)?.as_slice(), data);
        }
        drop(list);
        let read_us = start.elapsed().as_micros();
        assert_eq!(b.live_bytes(), 0);
        for (label, layout) in [
            ("classic", SaveLayout::Classic),
            ("xref", SaveLayout::XrefStream),
            ("objects", SaveLayout::XrefStreamWithObjectStreams),
        ] {
            let start = std::time::Instant::now();
            let bytes = d.save_to_bytes_with_layout(Default::default(), layout)?;
            let save_us = start.elapsed().as_micros();
            let r = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
            let owner =
                r.associated_file_owner(if axis == "owners" { Some(0) } else { None }, &b)?;
            assert_eq!(r.associated_files(&owner, &b)?.files().len(), count);
            assert_eq!(b.live_bytes(), 0);
            println!(
                "{{\"axis\":\"{axis}\",\"entries\":{count},\"payload_bytes\":{size},\"owners\":{owners},\"layout\":\"{label}\",\"edit_us\":{edit_us},\"read_us\":{read_us},\"save_us\":{save_us},\"bytes\":{},\"tracked_peak\":{},\"retained\":0}}",
                bytes.len(),
                b.peak_bytes()
            );
        }
    }
    Ok(())
}

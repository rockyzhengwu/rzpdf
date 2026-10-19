//! Fixed-size debug baseline; elapsed time is not a cross-version speed claim.
use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        optional_content_edit::{LayerBaseState, LayerConfiguration, LayerIntent},
        save_options::SaveLayout,
    },
    parser::syntax::ParseMode,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for count in [1, 100, 1000] {
        let d = Document::new()?;
        d.add_page(0)?;
        let b = OperationBudget::new(128 * 1024 * 1024);
        let start = std::time::Instant::now();
        let mut groups = Vec::new();
        for i in 0..count {
            groups.push(d.create_layer(&format!("Layer {i}"), LayerIntent::View, &b)?);
        }
        let created = start.elapsed().as_micros();
        let start = std::time::Instant::now();
        let target = d.layer_configuration_target(None)?;
        d.write_layer_configuration(
            Some(&target),
            &LayerConfiguration {
                name: Some("Baseline".into()),
                base: LayerBaseState::On,
                on: groups[..1].to_vec(),
                off: groups[1..].to_vec(),
            },
            &b,
        )?;
        d.append_layer_content(0, &groups[0], b"0 0 20 20 re f", &b)?;
        let edited = start.elapsed().as_micros();
        for (label, layout) in [
            ("classic", SaveLayout::Classic),
            ("xref", SaveLayout::XrefStream),
            ("objects", SaveLayout::XrefStreamWithObjectStreams),
        ] {
            let start = std::time::Instant::now();
            let out = d.save_to_bytes_with_layout(Default::default(), layout)?;
            let elapsed = start.elapsed().as_micros();
            let r = Document::from_bytes_with_mode(out.clone(), ParseMode::Strict)?;
            assert_eq!(r.optional_content()?.unwrap().groups.len(), count);
            assert_eq!(b.live_bytes(), 0);
            println!(
                "{{\"groups\":{count},\"layout\":\"{label}\",\"create_us\":{created},\"edit_us\":{edited},\"save_us\":{elapsed},\"bytes\":{},\"edit_tracked_peak\":{},\"edit_live\":0}}",
                out.len(),
                b.peak_bytes()
            );
        }
    }
    Ok(())
}

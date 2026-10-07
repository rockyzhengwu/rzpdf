//! Inspect a standalone TrueType Unicode glyph, not a PDF encoding/GID resolver.
use pdf::{
    error::{PdfError, PdfResult},
    font::truetype::{
        tables::{glyf::outline_bounds, outline_hit::outline_location},
        true_type_program::TrueTypeProgram,
    },
    parser::options::Limits,
};
fn main() -> PdfResult<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("Expected font path and one character".into()))?;
    let text = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("Expected one character".into()))?;
    let mut chars = text.chars();
    let character = chars
        .next()
        .ok_or_else(|| PdfError::DocumentError("Empty character".into()))?;
    let grid = match args.next() {
        None => false,
        Some(flag) if flag == "--grid" => true,
        Some(_) => return Err(PdfError::DocumentError("Expected optional --grid".into())),
    };
    if chars.next().is_some() || args.next().is_some() {
        return Err(PdfError::DocumentError(
            "Expected exactly one character".into(),
        ));
    }
    let limits = Limits::default();
    let length = std::fs::metadata(&path)?.len();
    if length > limits.max_decoded_bytes as u64 {
        return Err(PdfError::ResourceLimit("Font input bytes".into()));
    }
    let data = std::fs::read(path)?;
    let program = TrueTypeProgram::try_new_with_limits(&data, limits)?;
    let gid = program
        .unicode_glyph_id(character)?
        .ok_or_else(|| PdfError::FontError("No Unicode glyph mapping".into()))?;
    let commands = TrueTypeProgram::checked_glyph_outline(&data, gid, limits)?;
    println!(
        "character={character} gid={gid} commands={}",
        commands.len()
    );
    if let Some(bounds) = outline_bounds(&commands, &limits)? {
        println!("bounds_em={bounds:?}");
        let center = [(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.];
        println!("center={:?}", outline_location(&commands, center, &limits)?);
        let left_middle = [bounds[0] + (bounds[2] - bounds[0]) * 0.05, center[1]];
        println!(
            "left_middle={left_middle:?} {:?}",
            outline_location(&commands, left_middle, &limits)?
        );
        println!(
            "outside={:?}",
            outline_location(&commands, [bounds[2] + 1., bounds[3] + 1.], &limits)?
        );
        if grid {
            use pdf::font::truetype::tables::outline_hit::OutlineLocation;
            // Fixed cell-center samples over the actual em bounding box.
            // 0=outside, 1=inside, 2=boundary; row-major, y increasing.
            let mut result = String::new();
            result
                .try_reserve_exact(17 * 17)
                .map_err(|_| PdfError::ResourceLimit("Outline grid allocation".into()))?;
            for y in 0..17 {
                for x in 0..17 {
                    let point = [
                        bounds[0] + (bounds[2] - bounds[0]) * (x as f64 + 0.5) / 17.,
                        bounds[1] + (bounds[3] - bounds[1]) * (y as f64 + 0.5) / 17.,
                    ];
                    result.push(match outline_location(&commands, point, &limits)? {
                        OutlineLocation::Outside => '0',
                        OutlineLocation::Inside => '1',
                        OutlineLocation::Boundary => '2',
                    });
                }
            }
            println!("grid17={result}");
        }
    }
    Ok(())
}

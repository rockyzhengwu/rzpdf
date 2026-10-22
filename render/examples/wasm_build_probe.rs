//! WA-02 final-link/Node probe. Browser acceptance is a separate gate.
use pdf::document::Document;
use render::render_device::SkiaRender;
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};

fn fixture() -> Vec<u8> {
    let content = b"1 1 1 rg 0 0 96 96 re f 1 0 0 rg 8 8 40 40 re f q /GS gs 0 0 1 rg 28 28 40 40 re f Q 0 0 0 rg BT /F1 16 Tf 8 76 Td (Wasm) Tj ET";
    let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    stream.extend_from_slice(content);
    stream.extend_from_slice(b"\nendstream");
    let objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Resources << /Font << /F1 4 0 R >> /ExtGState << /GS 5 0 R >> >> /Contents 6 0 R >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        b"<< /Type /ExtGState /ca 0.5 >>".to_vec(),
        stream,
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = Document::from_bytes(fixture())?;
    let mut renderer = SkiaRender::new(72., 72.)?;
    document.get_page(0)?.display(&mut renderer)?;
    // Emscripten uses its in-memory filesystem; no host filesystem is mounted.
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/rzpdf-wasm-probe.png".into());
    renderer.save(&path)?;
    let png = std::fs::read(&path)?;
    let image = Image::from_encoded(Data::new_copy(&png)).ok_or("PNG decode failed")?;
    assert_eq!(image.dimensions(), (96, 96).into());
    let info = ImageInfo::new((96, 96), ColorType::RGBA8888, AlphaType::Unpremul, None);
    let mut pixels = vec![0u8; 96 * 96 * 4];
    assert!(image.read_pixels(&info, &mut pixels, 96 * 4, (0, 0), CachingHint::Disallow));
    let red = pixels
        .chunks_exact(4)
        .filter(|p| p[0] > 240 && p[1] < 16 && p[2] < 16)
        .count();
    let mixed = pixels
        .chunks_exact(4)
        .filter(|p| (110..145).contains(&p[0]) && p[1] < 16 && (110..145).contains(&p[2]))
        .count();
    let ink = pixels[..30 * 96 * 4]
        .chunks_exact(4)
        .filter(|p| p[0] < 64 && p[1] < 64 && p[2] < 64)
        .count();
    assert!(red > 500, "missing red geometry: {red}");
    assert!(mixed > 100, "missing transparent overlap: {mixed}");
    assert!(ink > 20, "missing font glyphs: {ink}");
    println!(
        "WA-02 runtime passed: 96x96 red={red} mixed={mixed} ink={ink} png_bytes={}",
        png.len()
    );
    Ok(())
}

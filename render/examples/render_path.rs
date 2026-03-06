use render::render_device::SkiaRender;

use std::fs::File;
use std::io::BufReader;

use pdf::document::PdfDocument;

fn main() {
    let docpath = "tests/path-test.pdf";
    let doc = PdfDocument::<BufReader<File>>::load(docpath).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let mut device = SkiaRender::new(300.0, 300.0);
    page.display(&mut device).unwrap();
    device.save("test.png");
}

use std::{fs::File, io::BufReader};

use pdf::{device::trace::TraceDevice, document::PdfDocument, objects};

fn main() {
    let docpath = "tests/path-test.pdf";
    let doc = PdfDocument::<BufReader<File>>::load(docpath).unwrap();
    let mut page = doc.get_page(2).unwrap();
    let mut device = TraceDevice::new();
    page.display(&mut device).unwrap();
    let s = device.to_xml();
    println!("{}", s);
}

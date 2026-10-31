//! Small reproducible scans with source dimensions typical of real documents.
use pdf::{
    document::{Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    parser::syntax::SyntaxParser,
};
use std::io::{Cursor, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .ok_or("usage: compatibility_fixtures OUTPUT")?;
    std::fs::create_dir_all(&output)?;
    for codec in ["flate", "ccitt", "jbig2"] {
        let stencil = codec != "flate";
        let doc = Document::new()?;
        doc.add_page(0)?;
        let (dict, data) = if stencil {
            let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
            for _ in 0..3500 {
                encoder.encode_line(
                    (0..2500).map(|x| {
                        if x < 1250 || (codec == "jbig2" && x % 4 == 0) {
                            fax::Color::Black
                        } else {
                            fax::Color::White
                        }
                    }),
                    2500,
                )?;
            }
            let encoded = encoder.finish()?.finish();
            if codec == "jbig2" {
                let mut page = Vec::new();
                for value in [2500u32, 3500, 0, 0] {
                    page.extend(value.to_be_bytes());
                }
                page.extend([0, 0, 0]);
                let mut region = page[..16].to_vec();
                region.extend([0, 1]);
                region.extend(encoded);
                let mut data = Vec::new();
                for (id, kind, association, body) in [
                    (1u32, 48, 1, page),
                    (2, 38, 1, region),
                    (3, 49, 1, vec![]),
                    (4, 51, 0, vec![]),
                ] {
                    data.extend(id.to_be_bytes());
                    data.extend([kind, 0, association]);
                    data.extend((body.len() as u32).to_be_bytes());
                    data.extend(body);
                }
                (
                    "<< /Type /XObject /Subtype /Image /Width 2500 /Height 3500 /ImageMask true /BitsPerComponent 1 /Decode [0 1] /Filter /JBIG2Decode >>",
                    data,
                )
            } else {
                (
                    "<< /Type /XObject /Subtype /Image /Width 2500 /Height 3500 /ImageMask true /BitsPerComponent 1 /Decode [0 1] /Filter /CCITTFaxDecode /DecodeParms << /K -1 /Columns 2500 /Rows 3500 >> >>",
                    encoded,
                )
            }
        } else {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            let row = vec![121; 2500 * 3];
            for _ in 0..3500 {
                encoder.write_all(&row)?;
            }
            (
                "<< /Type /XObject /Subtype /Image /Width 2500 /Height 3500 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode >>",
                encoder.finish()?,
            )
        };
        let mut parser = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            dict.as_bytes().to_vec(),
        ))?);
        let dict = parser.get_object()?.into_dict().ok_or("dictionary")?;
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, data)))?;
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)?;
        doc.replace_page_contents(
            0,
            format!("0.7 0.8 0.9 rg 0 0 612 792 re f 1 0 0 rg q 600 0 0 780 0 0 cm /{name} Do Q")
                .into_bytes(),
        )?;
        std::fs::write(format!("{output}/scan-{codec}.pdf"), doc.save_to_bytes()?)?;
    }
    Ok(())
}

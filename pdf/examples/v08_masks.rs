//! Graphics soft-mask corpus; no external assets.
use pdf::{
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions, resource_manager::ResourceKind,
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
};
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected new output PDF".into()))?;
    let doc = PdfDocument::new()?;
    for index in 0..10 {
        let dictionary=parse("<< /Subtype /Form /BBox [0 0 45 40] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources << /ExtGState << /Half << /ca 0.5 >> /Multiply << /BM /Multiply >> >> >> >>").into_dict().unwrap();
        let data = if index == 9 {
            b"/Multiply gs 1 0 0 rg 0 0 15 40 re f /Half gs 0 1 0 rg 15 0 15 40 re f".to_vec()
        } else {
            b"1 0 0 rg 0 0 15 40 re f /Half gs 0 1 0 rg 15 0 15 40 re f".to_vec()
        };
        let gid = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dictionary, data,
        )?))?;
        let subtype = if [2, 3, 9].contains(&index) {
            "Luminosity"
        } else {
            "Alpha"
        };
        let background = if index == 9 {
            "/BC [0.5 0.5 0.5]"
        } else if index == 3 {
            "/BC [1 1 1]"
        } else {
            ""
        };
        let transfer = if index == 1 {
            "/TR << /FunctionType 2 /Domain [0 1] /N 2 >>"
        } else {
            ""
        };
        let resources=parse(&format!("<< /ExtGState << /M << /SMask << /S /{subtype} /G {} 0 R {background} {transfer} >> >> /Clear << /SMask /None >> /Half << /ca 0.5 >> >> >>",gid.obj_num())).into_dict().unwrap();
        doc.add_page_with_builder(
            index,
            PageBuilder::new().size(60., 40.).resources(resources),
        )?;
        let code=match index {
            4=>b"0 0 1 rg q 1 0 0 1 5 0 cm /M gs 1 0 0 1 -5 0 cm 0 0 60 20 re f /Clear gs 0 20 15 20 re f Q 45 20 15 20 re f".to_vec(),
            5=>{let font=doc.resources().embed_truetype_font(index,include_bytes!("../tests/fixtures/v03/RzpdfTestSans-composite.ttf").to_vec(),"Ca",TrueTypeEmbeddingOptions{postscript_name:"RzpdfTestSans".into(),cap_height:700.,stem_v:80.,serif:false})?;
                let name=font.register_on_page(&doc,index)?;let bytes=font.encoding().encode_text("Ca")?;
                let hex=bytes.iter().map(|v|format!("{v:02X}")).collect::<String>();
                format!("/M gs 0 0 1 rg BT /{name} 38 Tf 1 0 0 1 0 5 Tm <{hex}> Tj ET").into_bytes()},
            6=>{let mut code=b"/M gs q 60 0 0 40 0 0 cm BI /W 2 /H 1 /BPC 8 /CS /RGB ID ".to_vec();code.extend_from_slice(&[255,0,0,0,0,255]);code.extend_from_slice(b" EI Q");code},
            7=>{let shade=parse("<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 60 0] /Extend [true true] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >>");let id=doc.try_insert_object(shade)?;let name=doc.resources().register_existing(index,ResourceKind::Shading,id)?;format!("/M gs /{name} sh").into_bytes()},
            8=>{let dict=parse("<< /Subtype /Form /BBox [0 0 60 40] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources << >> >>").into_dict().unwrap();let id=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dict,b"1 0 0 rg 0 0 45 40 re f 0 0 1 rg 20 0 40 40 re f".to_vec())?))?;let name=doc.resources().register_existing(index,ResourceKind::Form,id)?;format!("/M gs /Half gs /{name} Do").into_bytes()},
            _=>b"/M gs 0 0 1 rg 0 0 60 40 re f".to_vec(),
        };
        doc.replace_page_contents(index, code)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&doc.save_to_bytes()?)?;
    Ok(())
}

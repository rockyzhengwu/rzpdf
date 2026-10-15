//! Generated profiles and three equivalent paint routes, without external ICC data.
#[path = "support/v08_cmyk.rs"]
mod cmyk;
use pdf::{
    color::{
        conversion::{ColorConversionContext, RenderingIntent},
        icc_profile::ICCProfile,
    },
    document::PdfDocument,
    error::PdfResult,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::Path,
};
fn write(path: &Path, data: &[u8]) -> PdfResult<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(data)?;
    Ok(())
}
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn main() -> PdfResult<()> {
    let directory = std::env::args_os().nth(1).expect("output directory");
    let out = Path::new(&directory);
    let profiles = [
        ("srgb", ICCProfile::srgb().to_bytes()?, vec![51, 128, 204]),
        (
            "adobe",
            moxcms::ColorProfile::new_adobe_rgb().encode().unwrap(),
            vec![51, 128, 204],
        ),
        ("gray", ICCProfile::gray_srgb()?.to_bytes()?, vec![128]),
        ("cmyk", cmyk::profile(), vec![51, 102, 153, 77]),
    ];
    let doc = PdfDocument::new()?;
    let mut rows = String::new();
    for (index, (name, bytes, samples)) in profiles.into_iter().enumerate() {
        write(&out.join(format!("{name}.icc")), &bytes)?;
        let profile = ICCProfile::try_new(&bytes)?;
        let n = profile.number_of_components();
        let values: Vec<f32> = samples.iter().map(|v| *v as f32 / 255.).collect();
        for intent in [
            RenderingIntent::Perceptual,
            RenderingIntent::RelativeColorimetric,
            RenderingIntent::Saturation,
            RenderingIntent::AbsoluteColorimetric,
        ] {
            let color = profile.convert(
                &values,
                &ColorConversionContext::default().with_intent(intent),
            )?;
            rows.push_str(&format!(
                "{name} {intent:?} {} {} {} {}\n",
                samples
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                color.r,
                color.g,
                color.b
            ));
        }
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            parse(&format!("<< /N {n} >>")).into_dict().unwrap(),
            bytes,
        )?))?;
        let space = format!("[/ICCBased {} {} R]", id.obj_num(), id.gen_num());
        let img=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse(&format!("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /Intent /RelativeColorimetric /ColorSpace {space} >>")).into_dict().unwrap(),samples)?))?;
        let numbers = values
            .iter()
            .map(f32::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let resources=parse(&format!("<< /ColorSpace << /C {space} >> /XObject << /Im {} {} R >> /Shading << /S << /ShadingType 2 /ColorSpace {space} /Coords [40 0 60 0] /Extend [true true] /Function << /FunctionType 2 /Domain [0 1] /C0 [{numbers}] /C1 [{numbers}] /N 1 >> >> >> /ExtGState << /R << /RI /RelativeColorimetric >> >> >>",img.obj_num(),img.gen_num())).into_dict().unwrap();
        doc.add_page_with_builder(
            index as u32,
            PageBuilder::new().size(60., 20.).resources(resources),
        )?;
        doc.replace_page_contents(index as u32,format!("/Perceptual ri q /R gs /C cs {numbers} sc 0 0 20 20 re f Q q 20 0 0 20 20 0 cm /Im Do Q q 40 0 20 20 re W n /S sh Q").into_bytes())?;
    }
    write(&out.join("samples.txt"), rows.as_bytes())?;
    write(&out.join("cms.pdf"), &doc.save_to_bytes()?)?;
    Ok(())
}

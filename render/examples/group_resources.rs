//! Reproducible local-group fixtures and native browser-budget diagnostics.
use pdf::{
    document::{Document, prepared_page::ImageDecodePolicy, resource_manager::ResourceKind},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use render::render_device::SkiaRender;
use serde_json::json;
use std::{io::Cursor, path::Path};
fn object(text: &str) -> PdfResult<PdfObject> {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(
        text.as_bytes().to_vec(),
    ))?)
    .get_object()
}
fn form(doc: &Document, attrs: &str, body: &[u8]) -> PdfResult<ObjectId> {
    doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        object(&format!("<< /Subtype /Form {attrs} >>"))?
            .into_dict()
            .ok_or_else(|| PdfError::ObjectError("fixture dictionary".into()))?,
        body.to_vec(),
    )?))
}
fn fixtures(out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    for kind in [
        "local-icc",
        "nested",
        "rotated",
        "soft-mask",
        "knockout",
        "large-group",
    ] {
        let doc = Document::new()?;
        doc.add_page_with_builder(0, PageBuilder::new().size(600., 840.))?;
        let mut profile = moxcms::ColorProfile::new_srgb();
        let linear = moxcms::ColorProfile::new_gray_with_gamma(1.).gray_trc;
        profile.red_trc = linear.clone();
        profile.green_trc = linear.clone();
        profile.blue_trc = linear;
        // moxcms 0.9.1's encoder always writes the current date, even when
        // creation_date_time is set. Normalize only the standard ICC header
        // timestamp in these generated test profiles; color tags are untouched.
        let mut profile_bytes = profile.encode()?;
        profile_bytes[24..36].copy_from_slice(&[0x07, 0xea, 0, 10, 0, 31, 0, 0, 0, 0, 0, 0]);
        let icc = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            object("<< /N 3 >>")?
                .into_dict()
                .ok_or("profile dictionary")?,
            profile_bytes,
        )?))?;
        let cs = format!("[/ICCBased {} 0 R]", icc.obj_num());
        let resources = "/ExtGState << /Half << /ca 0.5 >> /Zero << /ca 0 >> >>";
        let paints = b"0 0 0 rg 0 0 100 40 re f /Half gs 1 1 1 rg 0 0 100 40 re f";
        let group=match kind{
            "nested"=>{
                let inner=form(&doc,&format!("/BBox [0 0 100 40] /Group << /S /Transparency /I false >> /Resources << {resources} >>"),paints)?;
                form(&doc,&format!("/BBox [0 0 160 80] /Group << /S /Transparency /I true /CS {cs} >> /Resources << /XObject << /Inner {} 0 R >> >>",inner.obj_num()),b"q 1 0 0 1 20 10 cm /Inner Do Q")?
            },
            "knockout"=>form(&doc,&format!("/BBox [0 0 100 40] /Group << /S /Transparency /I false /K true >> /Resources << {resources} >>"),b"1 0 0 rg 0 0 100 40 re f /Half gs 0 0 1 rg 30 0 60 40 re f /Zero gs 0 1 0 rg 50 5 10 25 re f")?,
            "large-group"=>form(&doc,&format!("/BBox [0 0 600 840] /Group << /S /Transparency /I true /CS {cs} >> /Resources << {resources} >>"),b"0.5 0.5 0.5 rg 0 0 600 840 re f")?,
            _=>form(&doc,&format!("/BBox [0 0 100 40] /Group << /S /Transparency /I true /CS {cs} >> /Resources << {resources} >>"),paints)?,
        };
        let mut name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, group)?;
        let transform = if kind == "rotated" {
            "0.8660254 0.5 -0.5 0.8660254 150 700"
        } else if kind == "large-group" {
            "1 0 0 1 0 0"
        } else {
            "1 0 0 1 60 760"
        };
        let mask_state = if kind == "soft-mask" {
            let mask = form(
                &doc,
                "/BBox [0 0 100 40] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources << /ExtGState << /Half << /ca 0.5 >> >> >>",
                b"/Half gs 1 1 1 rg 0 0 100 40 re f",
            )?;
            let state = doc.try_insert_object(object(&format!(
                "<< /SMask << /S /Alpha /G {} 0 R >> >>",
                mask.obj_num()
            ))?)?;
            let wrapper = form(
                &doc,
                &format!(
                    "/BBox [0 0 100 40] /Resources << /XObject << /Child {} 0 R >> /ExtGState << /Soft {} 0 R >> >>",
                    group.obj_num(),
                    state.obj_num()
                ),
                b"/Soft gs /Child Do",
            )?;
            name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, wrapper)?;
            String::new()
        } else {
            String::new()
        };
        doc.replace_page_contents(0,format!("0.7 0.8 0.9 rg 0 0 600 840 re f q {transform} cm {mask_state} /{name} Do Q 0 1 0 rg 20 20 40 40 re f").into_bytes())?;
        std::fs::write(out.join(format!("{kind}.pdf")), doc.save_to_bytes()?)?;
    }
    Ok(())
}
fn diagnose(input: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let limits = Limits {
        max_image_pixels: 4_000_000,
        max_render_pixels: 4_000_000,
        max_render_bytes: 16_000_000,
        max_decoded_bytes: 16 * 1024 * 1024,
        max_operation_bytes: 64 * 1024 * 1024,
        max_cached_stream_bytes: 16 * 1024 * 1024,
        ..Default::default()
    };
    let doc = Document::from_bytes_with_options(
        std::fs::read(input)?,
        ParseOptions {
            mode: ParseMode::Compatible,
            limits,
        },
    )?;
    std::fs::create_dir_all(out)?;
    let mut renders = Vec::new();
    for dpi in [72., 96., 102., 108., 144., 288.] {
        let result = (|| -> PdfResult<_> {
            let mut renderer = SkiaRender::with_limits(dpi, dpi, limits)?;
            doc.prepare_page_for_render_with_policy(0, None, dpi, ImageDecodePolicy::Budgeted)?
                .replay(&doc, &mut renderer)?;
            let image = renderer.to_rgba()?;
            renderer.save(
                out.join(format!("{}.png", dpi as u32))
                    .to_str()
                    .ok_or_else(|| PdfError::ObjectError("output path".into()))?,
            )?;
            Ok((image.width, image.height))
        })();
        renders.push(match result {
            Ok((width, height)) => {
                json!({"dpi":dpi,"status":"passed","width":width,"height":height})
            }
            Err(error) => {
                let resource = match error.root_cause() {
                    PdfError::ResourceLimitExceeded {
                        reason,
                        stage,
                        limit,
                        actual,
                    } => Some(json!({"reason":reason,"stage":stage,"limit":limit,"actual":actual})),
                    _ => None,
                };
                json!({"dpi":dpi,"status":"failed","message":error.to_string(),"resource":resource})
            }
        });
    }
    let report = json!({"input":input,"maxRenderPixels":limits.max_render_pixels,"maxRenderBytes":limits.max_render_bytes,"renders":renders});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("fixtures") if args.len() == 3 => fixtures(Path::new(&args[2])),
        Some("render") if args.len() == 4 => diagnose(Path::new(&args[2]), Path::new(&args[3])),
        _ => Err("usage: group_resources fixtures OUTPUT | render INPUT.pdf OUTPUT".into()),
    }
}

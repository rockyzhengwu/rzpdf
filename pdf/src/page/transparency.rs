//! Explicit transparency-group boundaries, separate from individual paints.
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
    page::{clip_path::ClipPath, general_state::BlendMode},
    pdf_context::PDFContext,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum SoftMaskKind {
    Alpha,
    Luminosity,
}

#[derive(Debug)]
pub struct SoftMaskPaint {
    pub list: super::display_list::DisplayList,
    pub kind: SoftMaskKind,
    pub background: [f32; 3],
    pub transfer: Option<crate::function::Function>,
    pub(crate) retained_bytes: usize,
    pub(crate) retained_segments: usize,
}
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct GroupPaint {
    /// None inherits the parent's working space. Some selects the actual
    /// isolated RGB matrix/TRC space, not an alias for DeviceRGB.
    pub blending_profile: Option<crate::color::icc_profile::ICCProfile>,
    pub rendering_intent: crate::color::conversion::RenderingIntent,
    pub isolated: bool,
    pub knockout: bool,
    pub alpha: f32,
    pub blend_mode: BlendMode,
    pub clip: ClipPath,
    pub soft_mask: Option<Arc<SoftMaskPaint>>,
}

#[derive(Debug, Clone)]
pub(crate) struct GroupParameters {
    pub blending_profile: Option<crate::color::icc_profile::ICCProfile>,
    pub gray_only: bool,
    pub isolated: bool,
    pub knockout: bool,
}

impl GroupParameters {
    pub(crate) fn parse<R: Read + Seek>(group: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let subtype = group.get("S").map(|v| ctx.resolve(v)).transpose()?;
        if subtype
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|n| n.name())
            != Some("Transparency")
        {
            return Err(PdfError::UnsupportedFeature("Form group subtype".into()));
        }
        let flag = |key: &str| -> PdfResult<bool> {
            group
                .get(key)
                .map(|v| {
                    ctx.resolve(v)?.as_bool().map(|v| v.value()).ok_or_else(|| {
                        PdfError::PageResourceError(format!("group {key} must be boolean"))
                    })
                })
                .transpose()
                .map(|v| v.unwrap_or(false))
        };
        let isolated = flag("I")?;
        let knockout = flag("K")?;
        // Nonisolated Form groups inherit their backdrop's blending space.
        // Their CS entry does not select a new space (ISO 32000, table 146).
        let mut gray_only = false;
        let mut blending_profile = None;
        if let Some(space) = group
            .get("CS")
            .filter(|_| isolated && !ctx.text_geometry_only())
        {
            use crate::color::{colorspace::ColorSpace, icc_profile::ICCProfile};
            let space = ColorSpace::try_new(space, ctx)?;
            let profile =
                match space {
                    ColorSpace::DeviceGray(_) => {
                        gray_only = true;
                        ICCProfile::srgb()
                    }
                    ColorSpace::DeviceRgb(_) => ICCProfile::srgb(),
                    ColorSpace::IccBased(space)
                        if space
                            .range()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .all(|r| *r == [0., 1.]) =>
                    {
                        space
                            .profile()
                            .filter(|profile| profile.is_rgb_blending_profile())
                            .cloned()
                            .ok_or_else(|| {
                                PdfError::UnsupportedFeature(
                                "transparency group ICC requires a usable RGB matrix/TRC profile"
                                    .into(),
                            )
                            })?
                    }
                    _ => return Err(PdfError::UnsupportedFeature(
                        "transparency group blending space outside RGB/Gray or matrix/TRC ICC RGB"
                            .into(),
                    )),
                };
            blending_profile = Some(profile);
        }
        Ok(Self {
            blending_profile,
            isolated,
            knockout,
            gray_only,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn indirect_icc_group_uses_profile_and_rejects_gray_or_nonunit_ranges() {
        use crate::{
            color::icc_profile::ICCProfile,
            document::Document,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let doc = Document::new().unwrap();
        let parse = |s: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
        };
        for (profile, dict, valid) in [
            (ICCProfile::srgb(), "<< /N 3 >>", true),
            (ICCProfile::gray_srgb().unwrap(), "<< /N 1 >>", false),
            (
                ICCProfile::srgb(),
                "<< /N 3 /Range [0 0.5 0 1 0 1] >>",
                false,
            ),
        ] {
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        parse(dict).into_dict().unwrap(),
                        profile.to_bytes().unwrap(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let cs = doc
                .try_insert_object(parse(&format!("[/ICCBased {} 0 R]", id.obj_num())))
                .unwrap();
            let group = parse(&format!(
                "<< /S /Transparency /I true /CS {} 0 R >>",
                cs.obj_num()
            ))
            .into_dict()
            .unwrap();
            let result = super::GroupParameters::parse(&group, doc.ctx());
            assert_eq!(result.is_ok(), valid, "{dict}");
            if valid {
                let profile = result.unwrap().blending_profile.unwrap();
                assert!(!profile.is_builtin_srgb());
                assert!(profile.is_rgb_blending_profile());
            }
        }
    }

    #[test]
    fn character_queries_skip_only_group_compositing_and_restore_render_validation() {
        use crate::{
            document::{Document, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let parse = |s: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
        };
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let dict = parse("<< /Subtype /Form /BBox [0 0 40 40] /Matrix [1 0 0 1 3 4] /Group << /S /Transparency /I true /CS /DeviceCMYK >> /Resources << /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >> >>").into_dict().unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, b"BT /F 10 Tf 2 8 Td (ABC) Tj ET".to_vec())
                    .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let mut page = doc.get_page(0).unwrap();
        assert!(page.display_list().is_err());
        let text = page.extract_characters_paint_order().unwrap();
        assert_eq!(
            text.iter()
                .filter_map(|c| c.unicode.as_deref())
                .collect::<String>(),
            "ABC"
        );
        assert_eq!(text[0].position.x(), 5.);
        assert_eq!(text[0].position.y(), 28.);
        assert!(page.display_list().is_err());
        doc.replace_page_contents(0, b"BT".to_vec()).unwrap();
        assert!(page.extract_characters_paint_order().is_err());
        assert!(!doc.ctx().text_geometry_only());
    }

    #[test]
    fn nonisolated_forms_inherit_blending_space_but_isolated_groups_validate_it() {
        use crate::{
            document::Document, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let doc = Document::new().unwrap();
        for (source, valid) in [
            ("<< /S /Transparency /CS /DeviceCMYK >>", true),
            ("<< /S /Transparency /I false /CS /DeviceRGB >>", true),
            ("<< /S /Transparency /I true /CS /DeviceRGB >>", true),
            ("<< /S /Transparency /I true /CS /DeviceCMYK >>", false),
            ("<< /S /Transparency /I 1 >>", false),
        ] {
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            assert_eq!(
                super::GroupParameters::parse(&dict, doc.ctx()).is_ok(),
                valid,
                "{source}"
            );
        }
    }
}

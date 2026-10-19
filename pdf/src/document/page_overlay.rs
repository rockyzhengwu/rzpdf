use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    page::{content_builder::ContentBuilder, content_parser::ContentParser},
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::{Cursor, Read, Seek};

/// Page-number text: prefix + number + optional separator/total + suffix.
/// Numbering follows the caller's selected page order, independently of PDF index.
pub struct PageNumberFormat<'a> {
    pub start: u32,
    pub prefix: &'a str,
    pub total_separator: Option<&'a str>,
    pub suffix: &'a str,
}

impl PageNumberFormat<'_> {
    /// Build Unicode text before encoding with the selected font. The displayed
    /// total is the selection length, not the last number or document page count.
    pub fn label(&self, ordinal: usize, total: usize, limits: Limits) -> PdfResult<String> {
        if ordinal >= total {
            return Err(PdfError::DocumentError(
                "Page-number ordinal outside selection".into(),
            ));
        }
        let number = u32::try_from(ordinal)
            .ok()
            .and_then(|n| self.start.checked_add(n))
            .ok_or_else(|| PdfError::ResourceLimit("Page number overflow".into()))?;
        let number = number.to_string();
        let total_text = self.total_separator.map(|_| total.to_string());
        let parts = [
            self.prefix,
            number.as_str(),
            self.total_separator.unwrap_or(""),
            total_text.as_deref().unwrap_or(""),
            self.suffix,
        ];
        let size = parts
            .iter()
            .try_fold(0usize, |size, part| size.checked_add(part.len()))
            .ok_or_else(|| PdfError::ResourceLimit("Page-number label size".into()))?;
        let chars = parts
            .iter()
            .try_fold(0usize, |size, part| size.checked_add(part.chars().count()))
            .ok_or_else(|| PdfError::ResourceLimit("Page-number character count".into()))?;
        if size > limits.max_token_bytes
            || size > limits.max_decoded_bytes
            || chars > limits.max_text_chars
        {
            return Err(PdfError::ResourceLimit("Page-number label budget".into()));
        }
        let mut label = String::new();
        label
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("Page-number allocation".into()))?;
        for part in parts {
            label.push_str(part);
        }
        Ok(label)
    }
}

/// Positioned text using a font already registered on the target page.
/// `codes` are font character codes, not UTF-8; use the font's encoder.
pub struct TextOverlay<'a> {
    pub font_name: &'a str,
    pub codes: &'a [u8],
    pub font_size: f32,
    /// PDF text matrix in unrotated default page user space.
    pub matrix: [f32; 6],
    pub color: [f32; 3],
}

// Unknown operators remain opaque. Inline images reject until their parser exists.
fn balanced_content(bytes: Vec<u8>, limits: Limits) -> PdfResult<()> {
    let mut parser = ContentParser::new(SyntaxParser::with_options(
        StreamReader::try_new(Cursor::new(bytes))?,
        ParseOptions {
            mode: ParseMode::Strict,
            limits,
        },
    ));
    let (mut graphics, mut marked, mut compatibility, mut text, mut operations) =
        (0usize, 0usize, 0usize, false, 0usize);
    let mut pending_clip = false;
    while let Some(operator) = parser.read_operator()? {
        if operations >= limits.max_content_operations {
            return Err(PdfError::ResourceLimit(
                "Overlay combined operations".into(),
            ));
        }
        operations += 1;
        if matches!(
            operator.name(),
            "q" | "Q" | "BT" | "ET" | "EMC" | "BX" | "EX" | "W" | "W*"
        ) && operator.num_operands() != 0
        {
            return Err(PdfError::ContentParseError(
                "Scope operator has unexpected operands".into(),
            ));
        }
        match operator.name() {
            "W" | "W*" => pending_clip = true,
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => pending_clip = false,
            _ => {}
        }
        match operator.name() {
            "q" => {
                graphics = graphics
                    .checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("Overlay graphics depth".into()))?
            }
            "Q" => {
                graphics = graphics.checked_sub(1).ok_or_else(|| {
                    PdfError::ContentParseError("Original content Q underflow".into())
                })?
            }
            "BT" if !text => text = true,
            "ET" if text => text = false,
            "BT" | "ET" => {
                return Err(PdfError::ContentParseError(
                    "Original text scope mismatch".into(),
                ));
            }
            "BMC" | "BDC" => marked += 1,
            "EMC" => {
                marked = marked.checked_sub(1).ok_or_else(|| {
                    PdfError::ContentParseError("Original marked-content underflow".into())
                })?
            }
            "BX" => compatibility += 1,
            "EX" => {
                compatibility = compatibility.checked_sub(1).ok_or_else(|| {
                    PdfError::ContentParseError("Original compatibility underflow".into())
                })?
            }
            _ => {}
        }
        if graphics > limits.max_depth
            || marked > limits.max_depth
            || compatibility > limits.max_depth
        {
            return Err(PdfError::ResourceLimit("Overlay content nesting".into()));
        }
    }
    if graphics != 0 || marked != 0 || compatibility != 0 || text || pending_clip {
        return Err(PdfError::ContentParseError(
            "Original content has unclosed scopes".into(),
        ));
    }
    Ok(())
}

impl<R: Read + Seek> PdfDocument<R> {
    fn encode_overlay_cid(
        &self,
        font: &super::embedded_font::RegisteredCidFont,
        text: &str,
    ) -> PdfResult<Vec<u8>> {
        let limits = self.ctx.limits();
        let chars = text.chars().count();
        let bytes = chars
            .checked_mul(2)
            .ok_or_else(|| PdfError::ResourceLimit("Overlay CID bytes overflow".into()))?;
        if chars > limits.max_text_chars
            || bytes > limits.max_decoded_bytes
            || bytes > limits.max_raw_stream_bytes
        {
            return Err(PdfError::ResourceLimit("Overlay CID text budget".into()));
        }
        font.encoding().encode_text(text)
    }

    /// Add the same Unicode watermark with an existing embedded CID font.
    /// The font must belong to this document and cover all text characters.
    pub fn watermark_pages_cid(
        &self,
        pages: &[u32],
        text: &str,
        font: &super::embedded_font::RegisteredCidFont,
        font_size: f32,
        matrix: [f32; 6],
        color: [f32; 3],
    ) -> PdfResult<()> {
        self.validate_overlay_selection(pages.iter().copied())?;
        if pages.is_empty() {
            return Ok(());
        }
        self.resolve_handle(font.object_handle())?;
        let codes = self.encode_overlay_cid(font, text)?;
        self.edit_transaction(|doc| {
            for index in pages {
                let name = font.register_on_page(doc, *index)?;
                doc.overlay_page_text(
                    *index,
                    TextOverlay {
                        font_name: &name,
                        codes: &codes,
                        font_size,
                        matrix,
                        color,
                    },
                )?;
            }
            Ok(())
        })
    }

    /// Number selected pages using an existing embedded CID font owned by this
    /// document. Its fixed repertoire must include every generated label.
    /// Page registrations and overlays roll back together on any failure.
    pub fn number_pages_cid(
        &self,
        pages: &[u32],
        format: &PageNumberFormat<'_>,
        font: &super::embedded_font::RegisteredCidFont,
        font_size: f32,
        matrix: [f32; 6],
        color: [f32; 3],
    ) -> PdfResult<()> {
        self.validate_overlay_selection(pages.iter().copied())?;
        if pages.is_empty() {
            return Ok(());
        }
        self.resolve_handle(font.object_handle())?;
        self.edit_transaction(|doc| {
            for (ordinal, index) in pages.iter().enumerate() {
                let label = format.label(ordinal, pages.len(), doc.ctx.limits())?;
                let codes = doc.encode_overlay_cid(font, &label)?;
                let name = font.register_on_page(doc, *index)?;
                doc.overlay_page_text(
                    *index,
                    TextOverlay {
                        font_name: &name,
                        codes: &codes,
                        font_size,
                        matrix,
                        color,
                    },
                )?;
            }
            Ok(())
        })
    }

    fn validate_overlay_selection(
        &self,
        pages: impl ExactSizeIterator<Item = u32>,
    ) -> PdfResult<()> {
        let size = pages.len();
        if size > self.ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("Overlay page selection".into()));
        }
        let count = self.try_page_count()?;
        let mut selected = std::collections::HashSet::new();
        selected
            .try_reserve(size)
            .map_err(|_| PdfError::ResourceLimit("Overlay selection allocation".into()))?;
        for index in pages {
            if index as usize >= count || !selected.insert(index) {
                return Err(PdfError::DocumentError(
                    "Invalid or duplicate overlay page".into(),
                ));
            }
        }
        Ok(())
    }

    /// Place the same opaque text watermark on selected pages atomically.
    /// Text is printable WinAnsi; the matrix controls placement and rotation.
    /// This is an overlay, not secure redaction or a transparency effect.
    pub fn watermark_pages_standard(
        &self,
        pages: &[u32],
        text: &str,
        font: super::resource_manager::StandardFont,
        font_size: f32,
        matrix: [f32; 6],
        color: [f32; 3],
    ) -> PdfResult<()> {
        self.validate_overlay_selection(pages.iter().copied())?;
        if pages.is_empty() {
            return Ok(());
        }
        let codes = font.encode_text_with_limits(text, self.ctx.limits())?;
        self.edit_transaction(|doc| {
            for index in pages {
                let name = doc.resources().register_standard_font(*index, font)?;
                doc.overlay_page_text(
                    *index,
                    TextOverlay {
                        font_name: &name,
                        codes: &codes,
                        font_size,
                        matrix,
                        color,
                    },
                )?;
            }
            Ok(())
        })
    }

    /// Number selected pages with an unembedded standard WinAnsi font. Registers
    /// each page's font and overlays its label in one transaction. For CJK use
    /// embedded font encoding with overlay_pages_text instead.
    pub fn number_pages_standard(
        &self,
        pages: &[u32],
        format: &PageNumberFormat<'_>,
        font: super::resource_manager::StandardFont,
        font_size: f32,
        matrix: [f32; 6],
        color: [f32; 3],
    ) -> PdfResult<()> {
        self.validate_overlay_selection(pages.iter().copied())?;
        if pages.is_empty() {
            return Ok(());
        }
        self.edit_transaction(|doc| {
            for (ordinal, index) in pages.iter().enumerate() {
                let label = format.label(ordinal, pages.len(), doc.ctx.limits())?;
                let codes = font.encode_text_with_limits(&label, doc.ctx.limits())?;
                let name = doc.resources().register_standard_font(*index, font)?;
                doc.overlay_page_text(
                    *index,
                    TextOverlay {
                        font_name: &name,
                        codes: &codes,
                        font_size,
                        matrix,
                        color,
                    },
                )?;
            }
            Ok(())
        })
    }

    /// Apply one positioned label per selected page atomically. Different font
    /// names/codes allow page-specific resources and numbering. Duplicate pages
    /// are rejected; an empty selection is a no-op.
    pub fn overlay_pages_text(&self, overlays: &[(u32, TextOverlay<'_>)]) -> PdfResult<()> {
        self.validate_overlay_selection(overlays.iter().map(|(index, _)| *index))?;
        if overlays.is_empty() {
            return Ok(());
        }
        self.edit_transaction(|doc| {
            for (index, text) in overlays {
                doc.overlay_page_text(
                    *index,
                    TextOverlay {
                        font_name: text.font_name,
                        codes: text.codes,
                        font_size: text.font_size,
                        matrix: text.matrix,
                        color: text.color,
                    },
                )?;
            }
            Ok(())
        })
    }

    /// Add positioned text for a watermark or page label. This does not register
    /// fonts, calculate alignment, rotate page coordinates, or remove content.
    pub fn overlay_page_text(&self, page_index: u32, text: TextOverlay<'_>) -> PdfResult<()> {
        if !text.font_size.is_finite()
            || text.font_size <= 0.0
            || text.matrix.iter().any(|value| !value.is_finite())
            || text
                .color
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(PdfError::DocumentError(
                "Invalid overlay text geometry/color".into(),
            ));
        }
        let location = self.page_location(page_index)?;
        let resources = super::page_tree::PageTree::inherited_attribute(
            &self.ctx,
            &crate::objects::PdfObject::PdfReference(
                crate::objects::pdf_reference::PdfReference::from_id(location.page_id),
            ),
            "Resources",
        )?
        .ok_or_else(|| PdfError::DocumentError("Overlay font resources missing".into()))?;
        let resources = resources
            .as_dict()
            .ok_or(PdfError::PageResourcesIsNotDict)?;
        let fonts = resources
            .get("Font")
            .ok_or_else(|| PdfError::DocumentError("Overlay fonts missing".into()))?;
        let fonts = self.ctx.resolve(fonts)?;
        let fonts = fonts
            .as_dict()
            .ok_or_else(|| PdfError::DocumentError("Overlay fonts must be a dictionary".into()))?;
        let font = fonts.get(text.font_name).ok_or_else(|| {
            PdfError::DocumentError("Overlay font is not registered on this page".into())
        })?;
        let font = self.ctx.resolve(font)?;
        let font = font
            .as_dict()
            .ok_or_else(|| PdfError::DocumentError("Overlay font must be a dictionary".into()))?;
        let kind = font
            .get("Type")
            .ok_or_else(|| PdfError::DocumentError("Overlay font Type missing".into()))?;
        if self.ctx.resolve(kind)?.as_name().map(|name| name.name()) != Some("Font") {
            return Err(PdfError::DocumentError(
                "Overlay resource is not a font".into(),
            ));
        }
        let mut content = ContentBuilder::new();
        content
            .set_fill_rgb(text.color[0], text.color[1], text.color[2])
            .begin_text();
        content.try_set_font_with_limits(text.font_name, text.font_size, self.ctx.limits())?;
        let [a, b, c, d, e, f] = text.matrix;
        content.set_text_matrix(a, b, c, d, e, f);
        content.show_encoded_text_with_limits(text.codes, self.ctx.limits())?;
        content.end_text();
        self.overlay_page_content(page_index, &content)
    }

    /// Append validated overlay in default page user space, independent of the
    /// original CTM/clip/colors. Not secure redaction and not a resource registrar.
    pub fn overlay_page_content(&self, page_index: u32, overlay: &ContentBuilder) -> PdfResult<()> {
        let limits = self.ctx.limits();
        let overlay = overlay.try_build_with_limits(limits)?;
        self.overlay_page_bytes(page_index, &overlay)
    }

    pub(crate) fn overlay_page_bytes(&self, page_index: u32, overlay: &[u8]) -> PdfResult<()> {
        self.ctx.require_content_edit_permission()?;
        let original = self.get_page(page_index)?.content_bytes()?;
        let limits = self.ctx.limits();
        let mut validation = Vec::new();
        validation
            .try_reserve_exact(original.len())
            .map_err(|_| PdfError::ResourceLimit("Overlay source validation allocation".into()))?;
        validation.extend_from_slice(&original);
        balanced_content(validation, limits)?;
        let prefix = b"q\n";
        let middle = b"\nQ\nn\nq\n";
        let suffix = b"\nQ\n";
        let size = original
            .len()
            .checked_add(overlay.len())
            .and_then(|n| n.checked_add(prefix.len() + middle.len() + suffix.len()))
            .ok_or_else(|| PdfError::ResourceLimit("Overlay bytes overflow".into()))?;
        if size > limits.max_raw_stream_bytes || size > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("Overlay combined bytes".into()));
        }
        let mut combined = Vec::new();
        combined
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("Overlay allocation".into()))?;
        combined.extend_from_slice(prefix);
        combined.extend_from_slice(&original);
        combined.extend_from_slice(middle);
        combined.extend_from_slice(overlay);
        combined.extend_from_slice(suffix);
        let mut validation = Vec::new();
        validation.try_reserve_exact(size).map_err(|_| {
            PdfError::ResourceLimit("Overlay combined validation allocation".into())
        })?;
        validation.extend_from_slice(&combined);
        balanced_content(validation, limits)?;
        self.replace_page_contents(page_index, combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_preserves_rotated_cropped_page_geometry() {
        let doc = PdfDocument::new().unwrap();
        for (index, rotation) in [0, 90, 180, 270].into_iter().enumerate() {
            let index = index as u32;
            doc.add_page(index).unwrap();
            doc.set_page_rotation(index, rotation).unwrap();
            doc.set_page_crop_box(
                index,
                Some(crate::geom::rectangle::Rectangle::new(
                    10.0, 20.0, 200.0, 300.0,
                )),
            )
            .unwrap();
            let before = doc.page_location(index).unwrap().page_dict;
            let mut content = ContentBuilder::new();
            content.rectangle(40.0, 50.0, 60.0, 70.0).stroke();
            doc.overlay_page_content(index, &content).unwrap();
            let after = doc.page_location(index).unwrap().page_dict;
            for key in ["Rotate", "MediaBox", "CropBox", "Parent", "Resources"] {
                assert_eq!(after.get(key), before.get(key), "changed {key}");
            }
            doc.get_page(index).unwrap().display_list().unwrap();
        }
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for index in 0..4 {
            let before = doc.page_location(index).unwrap().page_dict;
            let after = reopened.page_location(index).unwrap().page_dict;
            for key in ["Rotate", "MediaBox", "CropBox"] {
                assert_eq!(after.get(key), before.get(key));
            }
            reopened.get_page(index).unwrap().display_list().unwrap();
        }
    }

    #[test]
    fn inherited_font_resources_work_and_wrong_resource_is_rejected() {
        use crate::objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName};
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, super::super::resource_manager::StandardFont::Helvetica)
            .unwrap();
        let location = doc.page_location(0).unwrap();
        let resources = location.page_dict.get("Resources").unwrap().clone();
        let (root_id, mut root) = doc.root_pages().unwrap();
        root.try_insert("Resources".into(), resources).unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        let mut page = location.page_dict;
        page.remove("Resources");
        doc.update_object(location.page_id, PdfObject::PdfDict(page))
            .unwrap();
        let overlay = |font_name| TextOverlay {
            font_name,
            codes: b"Page",
            font_size: 12.0,
            matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
            color: [0.0; 3],
        };
        doc.overlay_page_text(0, overlay(&name)).unwrap();
        let revision = doc.revision();
        assert!(doc.overlay_page_text(0, overlay("Missing")).is_err());
        assert_eq!(doc.revision(), revision);
        let mut wrong = PdfDict::default();
        wrong
            .try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("XObject".into())),
            )
            .unwrap();
        let mut fonts = PdfDict::default();
        fonts
            .try_insert("Wrong".into(), PdfObject::PdfDict(wrong))
            .unwrap();
        let mut resources = PdfDict::default();
        resources
            .try_insert("Font".into(), PdfObject::PdfDict(fonts))
            .unwrap();
        let mut page = doc.page_location(0).unwrap().page_dict;
        page.try_insert("Resources".into(), PdfObject::PdfDict(resources))
            .unwrap();
        doc.update_object(location.page_id, PdfObject::PdfDict(page.clone()))
            .unwrap();
        let revision = doc.revision();
        assert!(doc.overlay_page_text(0, overlay("Wrong")).is_err());
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.page_location(0).unwrap().page_dict, page);
    }

    #[test]
    fn text_overlay_requires_registered_font_before_mutation() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let revision = doc.revision();
        assert!(
            doc.overlay_page_text(
                0,
                TextOverlay {
                    font_name: "Missing",
                    codes: b"Page",
                    font_size: 12.0,
                    matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
                    color: [0.0; 3],
                }
            )
            .is_err()
        );
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn standard_watermark_is_atomic_and_encodes_printable_text() {
        use super::super::resource_manager::StandardFont;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let matrix = [0.7, 0.7, -0.7, 0.7, 100.0, 200.0];
        let revision = doc.revision();
        assert!(
            doc.watermark_pages_standard(
                &[0, 1],
                "中文",
                StandardFont::Helvetica,
                40.0,
                matrix,
                [0.7; 3]
            )
            .is_err()
        );
        assert_eq!(doc.revision(), revision);
        doc.replace_page_contents(1, b"Q".to_vec()).unwrap();
        let before: Vec<_> = (0..2)
            .map(|i| doc.page_location(i).unwrap().page_dict)
            .collect();
        assert!(
            doc.watermark_pages_standard(
                &[0, 1],
                "DRAFT",
                StandardFont::Helvetica,
                40.0,
                matrix,
                [0.7; 3]
            )
            .is_err()
        );
        for i in 0..2 {
            assert_eq!(doc.page_location(i).unwrap().page_dict, before[i as usize]);
        }
        doc.replace_page_contents(1, Vec::new()).unwrap();
        doc.watermark_pages_standard(
            &[0, 1],
            "DRAFT",
            StandardFont::Helvetica,
            40.0,
            matrix,
            [0.7; 3],
        )
        .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for index in 0..2 {
            assert!(
                String::from_utf8(reopened.get_page(index).unwrap().content_bytes().unwrap())
                    .unwrap()
                    .contains("4452414654")
            );
            reopened.get_page(index).unwrap().display_list().unwrap();
        }
    }

    #[test]
    fn standard_numbering_registers_fonts_and_rolls_back_late_failure() {
        use super::super::resource_manager::StandardFont;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        doc.replace_page_contents(1, b"Q".to_vec()).unwrap();
        let before: Vec<_> = (0..2)
            .map(|i| doc.page_location(i).unwrap().page_dict)
            .collect();
        let format = PageNumberFormat {
            start: 1,
            prefix: "Page ",
            total_separator: Some(" / "),
            suffix: "",
        };
        let matrix = [1.0, 0.0, 0.0, 1.0, 48.0, 48.0];
        assert!(
            doc.number_pages_standard(
                &[0, 1],
                &format,
                StandardFont::Helvetica,
                12.0,
                matrix,
                [0.0; 3]
            )
            .is_err()
        );
        for i in 0..2 {
            assert_eq!(doc.page_location(i).unwrap().page_dict, before[i as usize]);
        }
        doc.replace_page_contents(1, Vec::new()).unwrap();
        doc.number_pages_standard(
            &[1, 0],
            &format,
            StandardFont::Helvetica,
            12.0,
            matrix,
            [0.0; 3],
        )
        .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for (index, expected) in [(1, "506167652031202F2032"), (0, "506167652032202F2032")] {
            assert!(
                String::from_utf8(reopened.get_page(index).unwrap().content_bytes().unwrap())
                    .unwrap()
                    .contains(expected)
            );
            reopened.get_page(index).unwrap().display_list().unwrap();
        }
    }

    #[test]
    fn page_number_format_preserves_unicode_and_checks_limits() {
        let format = PageNumberFormat {
            start: 1,
            prefix: "第 ",
            total_separator: Some(" 页 / 共 "),
            suffix: " 页",
        };
        let label = format.label(1, 10, Limits::default()).unwrap();
        assert_eq!(label, "第 2 页 / 共 10 页");
        assert!(format.label(10, 10, Limits::default()).is_err());
        assert!(format.label(0, 0, Limits::default()).is_err());
        assert!(
            format
                .label(
                    1,
                    10,
                    Limits {
                        max_token_bytes: label.len() - 1,
                        ..Limits::default()
                    }
                )
                .is_err()
        );
        assert!(
            format
                .label(
                    1,
                    10,
                    Limits {
                        max_text_chars: label.chars().count() - 1,
                        ..Limits::default()
                    }
                )
                .is_err()
        );
        let overflow = PageNumberFormat {
            start: u32::MAX,
            prefix: "",
            total_separator: None,
            suffix: "",
        };
        assert!(overflow.label(1, 2, Limits::default()).is_err());
        assert_eq!(
            overflow.label(0, 1, Limits::default()).unwrap(),
            "4294967295"
        );
    }

    #[test]
    fn batch_text_overlay_is_atomic_and_validates_selection() {
        let doc = PdfDocument::new().unwrap();
        let mut fonts = Vec::new();
        for index in 0..2 {
            doc.add_page(index).unwrap();
            fonts.push(
                doc.resources()
                    .register_standard_font(
                        index,
                        super::super::resource_manager::StandardFont::Helvetica,
                    )
                    .unwrap(),
            );
        }
        let label = |index: usize| TextOverlay {
            font_name: &fonts[index],
            codes: b"Page",
            font_size: 12.0,
            matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
            color: [0.0; 3],
        };
        let revision = doc.revision();
        doc.overlay_pages_text(&[]).unwrap();
        assert!(
            doc.overlay_pages_text(&[(0, label(0)), (0, label(0))])
                .is_err()
        );
        assert!(doc.overlay_pages_text(&[(2, label(0))]).is_err());
        assert_eq!(doc.revision(), revision);
        doc.replace_page_contents(1, b"Q".to_vec()).unwrap();
        let first = doc.page_location(0).unwrap().page_dict;
        let second = doc.page_location(1).unwrap().page_dict;
        let original = doc.get_page(0).unwrap().content_bytes().unwrap();
        assert!(
            doc.overlay_pages_text(&[(0, label(0)), (1, label(1))])
                .is_err()
        );
        assert_eq!(doc.page_location(0).unwrap().page_dict, first);
        assert_eq!(doc.page_location(1).unwrap().page_dict, second);
        assert_eq!(doc.get_page(0).unwrap().content_bytes().unwrap(), original);
        doc.replace_page_contents(1, Vec::new()).unwrap();
        doc.overlay_pages_text(&[(1, label(1)), (0, label(0))])
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for index in 0..2 {
            reopened.get_page(index).unwrap().display_list().unwrap();
        }
    }

    #[test]
    fn positioned_text_overlay_roundtrips_and_rejects_invalid_geometry() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, super::super::resource_manager::StandardFont::Helvetica)
            .unwrap();
        let text = |size| TextOverlay {
            font_name: &font,
            codes: b"Page 1",
            font_size: size,
            matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
            color: [0.5; 3],
        };
        let revision = doc.revision();
        assert!(doc.overlay_page_text(0, text(f32::NAN)).is_err());
        assert_eq!(doc.revision(), revision);
        doc.overlay_page_text(0, text(12.0)).unwrap();
        doc.get_page(0).unwrap().display_list().unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        reopened.get_page(0).unwrap().display_list().unwrap();
        assert!(
            String::from_utf8(reopened.get_page(0).unwrap().content_bytes().unwrap())
                .unwrap()
                .contains("506167652031")
        );
    }

    #[test]
    fn overlay_is_isolated_and_preserves_original_shared_content() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(
            0,
            b"2 0 0 2 10 20 cm 0 0 10 10 re W n 1 0 0 rg 5 5 m".to_vec(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let original = doc.get_page(1).unwrap().content_bytes().unwrap();
        let mut overlay = ContentBuilder::new();
        overlay.rectangle(40.0, 50.0, 60.0, 70.0).stroke();
        doc.overlay_page_content(0, &overlay).unwrap();
        let combined = doc.get_page(0).unwrap().content_bytes().unwrap();
        assert!(String::from_utf8(combined).unwrap().contains("\nQ\nn\nq\n"));
        assert_eq!(doc.get_page(1).unwrap().content_bytes().unwrap(), original);
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let paths: Vec<_> = list
            .objects()
            .iter()
            .filter_map(|object| match object {
                crate::page::page_object::PageObject::Path(path) if path.stroke() => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].matrix().a().abs(), 1.0);
        assert_eq!(paths[0].matrix().d().abs(), 1.0);
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        reopened.get_page(0).unwrap().display_list().unwrap();
    }
    #[test]
    fn overlay_wrapper_budgets_and_outer_rollback() {
        let empty = ContentBuilder::new();
        // Empty original and overlay still require five wrapper operators.
        for (operations, bytes, succeeds) in [(4, 1024, false), (5, 11, false), (5, 12, true)] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_content_operations: operations,
                    max_raw_stream_bytes: bytes,
                    ..Limits::default()
                },
                ..ParseOptions::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            let before = doc.page_location(0).unwrap().page_dict;
            let revision = doc.revision();
            assert_eq!(doc.overlay_page_content(0, &empty).is_ok(), succeeds);
            if !succeeds {
                assert_eq!(doc.revision(), revision);
                assert_eq!(doc.page_location(0).unwrap().page_dict, before);
            }
        }
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"% trailing comment".to_vec())
            .unwrap();
        let before = doc.page_location(0).unwrap().page_dict;
        let original = doc.get_page(0).unwrap().content_bytes().unwrap();
        let result: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.overlay_page_content(0, &empty)?;
            doc.get_page(0)?.display_list()?;
            Err(PdfError::DocumentError("overlay rollback test".into()))
        });
        assert!(result.is_err());
        assert_eq!(doc.page_location(0).unwrap().page_dict, before);
        assert_eq!(doc.get_page(0).unwrap().content_bytes().unwrap(), original);
    }

    #[test]
    fn unbalanced_original_content_rejects_before_mutation() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let overlay = ContentBuilder::new();
        for bytes in [
            b"Q".as_slice(),
            b"q",
            b"BT",
            b"ET",
            b"/X BMC",
            b"EMC",
            b"BX",
            b"EX",
            b"0 0 10 10 re W",
        ] {
            doc.replace_page_contents(0, bytes.to_vec()).unwrap();
            let revision = doc.revision();
            assert!(doc.overlay_page_content(0, &overlay).is_err());
            assert_eq!(doc.revision(), revision);
        }
    }
}

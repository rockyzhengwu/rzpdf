use super::resource_manager::{ResourceKind, ResourceManager};
use crate::{
    error::{PdfError, PdfResult},
    font::{cid_encoding::UnicodeCidEncoding, truetype::true_type_program::TrueTypeProgram},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

/// Caller-supplied metrics are in PDF's 1000-unit space, not font design units.
pub struct TrueTypeEmbeddingOptions {
    pub postscript_name: String,
    pub cap_height: f64,
    pub stem_v: f64,
    pub serif: bool,
}

pub struct RegisteredCidFont {
    /// Name in the page scope used for the initial registration.
    pub resource_name: String,
    encoding: UnicodeCidEncoding,
    handle: super::identity::ObjectHandle,
}

impl RegisteredCidFont {
    pub fn encoding(&self) -> &UnicodeCidEncoding {
        &self.encoding
    }

    pub fn object_handle(&self) -> &super::identity::ObjectHandle {
        &self.handle
    }

    /// Reuse this font in another page of its owning document. The returned
    /// name is page-local; it may differ from the initial resource_name.
    /// Handles created inside an outer transaction must not escape a rollback.
    pub fn register_on_page<R: Read + Seek>(
        &self,
        document: &super::PdfDocument<R>,
        page_index: u32,
    ) -> PdfResult<String> {
        document
            .resources()
            .register_handle(page_index, ResourceKind::Font, &self.handle)
    }
}

fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn number(value: f64) -> PdfResult<PdfObject> {
    Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
        value.to_string().as_bytes(),
    )?))
}
fn reference(id: crate::objects::object_id::ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

impl<R: Read + Seek> ResourceManager<'_, R> {
    /// Embed a whole static TrueType font with an immutable Identity-H repertoire.
    /// Technical fsType checks do not replace the caller's font license review.
    /// No subsetting, shaping, variable instances, or CFF embedding is performed.
    pub fn embed_truetype_font(
        &self,
        page_index: u32,
        data: Vec<u8>,
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
    ) -> PdfResult<RegisteredCidFont> {
        self.document.page_location(page_index)?;
        if self.document.effective_version()? < super::save_options::PdfVersion::V1_3 {
            return Err(PdfError::UnsupportedFeature(
                "CIDFontType2 embedding requires PDF 1.3".into(),
            ));
        }
        let limits = self.document.ctx.limits();
        if data.len() > limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("FontFile2 bytes".into()));
        }
        if options.postscript_name.is_empty()
            || options.postscript_name.len() > 127
            || !options
                .postscript_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || !options.cap_height.is_finite()
            || options.cap_height < 0.0
            || !options.stem_v.is_finite()
            || options.stem_v <= 0.0
        {
            return Err(PdfError::FontError(
                "Invalid supplied FontDescriptor metadata".into(),
            ));
        }
        let program = TrueTypeProgram::try_new_with_limits(&data, limits)?;
        program.validate_outline_embedding(false, true)?;
        TrueTypeProgram::checked_glyph_locations(&data, limits)?;
        // Default-instance metadata must not masquerade as variable-font embedding.
        let sfnt = crate::font::truetype::sfnt::Sfnt::try_new_with_limits(
            &mut crate::font::truetype::reader::TrueTypeReader::new(&data),
            limits,
        )?;
        if sfnt.get_table("fvar").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "Variable font embedding".into(),
            ));
        }
        let metrics = program.horizontal_metrics().ok_or_else(|| {
            PdfError::FontError("FontDescriptor requires horizontal metrics".into())
        })?;
        let post = program
            .post_metrics()
            .ok_or_else(|| PdfError::FontError("FontDescriptor requires post metrics".into()))?;
        let encoding = UnicodeCidEncoding::from_text(&program, repertoire, limits)?;
        let to_unicode = encoding.to_unicode_cmap()?;
        let gid_map = encoding.cid_to_gid_map()?;
        let scale = 1000.0 / f64::from(metrics.units_per_em());
        let mut descriptor = PdfDict::default();
        descriptor.try_insert("Type".into(), name("FontDescriptor"))?;
        descriptor.try_insert("FontName".into(), name(&options.postscript_name))?;
        let flags = 4
            | u32::from(post.is_fixed_pitch())
            | if options.serif { 2 } else { 0 }
            | if post.italic_angle() != 0.0 { 64 } else { 0 };
        for (key, value) in [
            ("Flags", f64::from(flags)),
            ("ItalicAngle", post.italic_angle()),
            ("Ascent", f64::from(metrics.ascent()) * scale),
            ("Descent", f64::from(metrics.descent()) * scale),
            ("CapHeight", options.cap_height),
            ("StemV", options.stem_v),
        ] {
            descriptor.try_insert(key.into(), number(value)?)?;
        }
        let mut bounds = PdfArray::default();
        for value in metrics.bounds() {
            bounds.try_add_obj(number(f64::from(value) * scale)?)?;
        }
        descriptor.try_insert("FontBBox".into(), PdfObject::PdfArray(bounds))?;
        let mut widths = PdfArray::default();
        for (_, width) in encoding.widths() {
            widths.try_add_obj(number(width)?)?;
        }
        let mut w = PdfArray::default();
        if widths.len() != 0 {
            w.try_add_obj(number(1.0)?)?;
            w.try_add_obj(PdfObject::PdfArray(widths))?;
        }
        let mut system = PdfDict::default();
        system.try_insert(
            "Registry".into(),
            PdfObject::PdfString(PdfString::new(b"Adobe".to_vec(), false)),
        )?;
        system.try_insert(
            "Ordering".into(),
            PdfObject::PdfString(PdfString::new(b"Identity".to_vec(), false)),
        )?;
        system.try_insert("Supplement".into(), number(0.0)?)?;
        let (resource_name, handle) = self.document.edit_transaction(|doc| {
            let mut font_file = PdfDict::default();
            font_file.try_insert("Length1".into(), number(data.len() as f64)?)?;
            let file_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(font_file, data)?,
            ))?;
            descriptor.try_insert("FontFile2".into(), reference(file_id))?;
            let descriptor_id = doc.try_insert_object(PdfObject::PdfDict(descriptor))?;
            let gid_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), gid_map)?,
            ))?;
            let unicode_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), to_unicode)?,
            ))?;
            let mut descendant = PdfDict::default();
            for (key, value) in [
                ("Type", "Font"),
                ("Subtype", "CIDFontType2"),
                ("BaseFont", &options.postscript_name),
            ] {
                descendant.try_insert(key.into(), name(value))?;
            }
            descendant.try_insert("CIDSystemInfo".into(), PdfObject::PdfDict(system))?;
            descendant.try_insert("FontDescriptor".into(), reference(descriptor_id))?;
            descendant.try_insert("CIDToGIDMap".into(), reference(gid_id))?;
            descendant.try_insert("DW".into(), number(0.0)?)?;
            descendant.try_insert("W".into(), PdfObject::PdfArray(w))?;
            let descendant_id = doc.try_insert_object(PdfObject::PdfDict(descendant))?;
            let mut descendants = PdfArray::default();
            descendants.try_add_obj(reference(descendant_id))?;
            let mut font = PdfDict::default();
            for (key, value) in [
                ("Type", "Font"),
                ("Subtype", "Type0"),
                ("BaseFont", &options.postscript_name),
                ("Encoding", "Identity-H"),
            ] {
                font.try_insert(key.into(), name(value))?;
            }
            font.try_insert("DescendantFonts".into(), PdfObject::PdfArray(descendants))?;
            font.try_insert("ToUnicode".into(), reference(unicode_id))?;
            let id = doc.try_insert_object(PdfObject::PdfDict(font))?;
            let resource_name =
                doc.resources()
                    .register_existing(page_index, ResourceKind::Font, id)?;
            Ok((resource_name, doc.object_handle(id)?))
        })?;
        Ok(RegisteredCidFont {
            resource_name,
            encoding,
            handle,
        })
    }
}

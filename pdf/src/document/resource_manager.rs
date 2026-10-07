use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
};
use std::io::{Read, Seek};

#[derive(Clone, Copy)]
pub enum ResourceKind {
    Font,
    Image,
    Form,
    ColorSpace,
}

/// Unembedded standard Latin fonts. Unicode text still needs font-specific encoding.
#[derive(Clone, Copy)]
pub enum StandardFont {
    Helvetica,
    TimesRoman,
    Courier,
}

impl StandardFont {
    /// Encode printable Unicode characters in the WinAnsi repertoire. Undefined
    /// slots and control characters are rejected; this is not a CJK encoder.
    pub fn encode_text(self, text: &str) -> PdfResult<Vec<u8>> {
        self.encode_text_with_limits(text, crate::parser::options::Limits::default())
    }

    pub fn encode_text_with_limits(
        self,
        text: &str,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<u8>> {
        let count = text.chars().count();
        if count > limits.max_text_chars || count > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("WinAnsi encoded text size".into()));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("WinAnsi text allocation".into()))?;
        for character in text.chars() {
            let code = if character.is_control() {
                None
            } else {
                u16::try_from(character as u32)
                    .ok()
                    .and_then(|value| {
                        crate::font::encoding::FontEncoding::WinAnsi.charcode_from_unicode(&value)
                    })
                    .and_then(|value| u8::try_from(value).ok())
            }
            .ok_or_else(|| {
                PdfError::UnsupportedFeature(format!(
                    "character U+{:04X} is not printable WinAnsi",
                    character as u32
                ))
            })?;
            output.push(code);
        }
        Ok(output)
    }

    fn base_name(self) -> &'static str {
        match self {
            Self::Helvetica => "Helvetica",
            Self::TimesRoman => "Times-Roman",
            Self::Courier => "Courier",
        }
    }
}

/// Registers existing objects in a page's resource scope without mutating shared dictionaries.
pub struct ResourceManager<'a, R: Read + Seek> {
    pub(crate) document: &'a PdfDocument<R>,
}

impl<R: Read + Seek> ResourceManager<'_, R> {
    /// Internal Form registration for a budgeted nested edit. Page-tree
    /// traversal, owned registration copies and transaction snapshot payloads
    /// use the caller's pool. Initial source-cache loading remains excluded.
    pub(super) fn register_form_with_budget(
        &self,
        page_index: u32,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(String, crate::budget::MemoryLease)> {
        self.document.edit_transaction_with_budget(budget, |doc| {
            let reference = PdfObject::PdfReference(PdfReference::from_id(id));
            let (_object_lease, object) = {
                let (object, lease) = doc.ctx.resolve_owned_with_budget(&reference, budget)?;
                (lease, object)
            };
            let (_form_lease, _form) = {
                let (form, lease) =
                    crate::page::pdf_form::PdfForm::try_new_with_budget(&object, &doc.ctx, budget)?;
                (lease, form)
            };
            let (_page_lease, location) = {
                let (location, lease) = doc
                    .page_location_with_budget(page_index, budget)?
                    .into_parts();
                (lease, location)
            };
            let page_id = location.page_id;
            let page = location.page_dict;
            let mut _resources_lease = None;
            let resources = match super::page_tree::PageTree::inherited_attribute_with_budget(
                &doc.ctx,
                &PdfObject::PdfReference(PdfReference::from_id(page_id)),
                "Resources",
                budget,
            )? {
                Some((value, lease)) => {
                    _resources_lease = Some(lease);
                    value.into_dict().ok_or(PdfError::PageResourcesIsNotDict)?
                }
                None => PdfDict::default(),
            };
            let mut _entries_lease = None;
            let entries = match resources.get("XObject") {
                Some(value) => {
                    let (value, lease) = doc.ctx.resolve_owned_with_budget(value, budget)?;
                    _entries_lease = Some(lease);
                    value.into_dict().ok_or_else(|| {
                        PdfError::ObjectError("resource category must be a dictionary".into())
                    })?
                }
                None => PdfDict::default(),
            };
            if let Some(name) = (&entries)
                .into_iter()
                .filter(|(_, value)| value.as_reference().is_some_and(|value| value.id() == id))
                .map(|(name, _)| name)
                .min()
            {
                return crate::budget::copy_string(name, budget);
            }
            let limits = doc.ctx.limits();
            if (&entries).into_iter().len() >= limits.max_objects {
                return Err(PdfError::ResourceLimit("page resource count".into()));
            }
            for index in 1..=limits.max_objects {
                let mut digits = [0u8; 20];
                let mut start = digits.len();
                let mut number = index;
                loop {
                    start -= 1;
                    digits[start] = b'0' + (number % 10) as u8;
                    number /= 10;
                    if number == 0 {
                        break;
                    }
                }
                let mut buffer = [0u8; 22];
                buffer[..2].copy_from_slice(b"Fm");
                let length = 2 + digits.len() - start;
                buffer[2..length].copy_from_slice(&digits[start..]);
                if length > limits.max_token_bytes {
                    return Err(PdfError::ResourceLimit("page resource name bytes".into()));
                }
                let candidate = std::str::from_utf8(&buffer[..length]).unwrap();
                if entries.contains_key(candidate) {
                    continue;
                }
                let (name, mut retained) = crate::budget::copy_string(candidate, budget)?;
                let (entries, _binding_lease) =
                    entries.with_replaced_entry_with_budget(&name, &reference, budget, limits)?;
                let (resources, _replacement_lease) = resources.with_replaced_entry_with_budget(
                    "XObject",
                    &PdfObject::PdfDict(entries),
                    budget,
                    limits,
                )?;
                let (page, mut page_lease) = page.with_replaced_entry_with_budget(
                    "Resources",
                    &PdfObject::PdfDict(resources),
                    budget,
                    limits,
                )?;
                retained.absorb(&mut page_lease)?;
                doc.update_object(page_id, PdfObject::PdfDict(page))?;
                return Ok((name, retained));
            }
            Err(PdfError::ResourceLimit("resource name exhausted".into()))
        })
    }

    /// Register an unembedded standard font with WinAnsiEncoding. Reuses an
    /// identical font already registered in this page's resource scope.
    pub fn register_standard_font(&self, page_index: u32, font: StandardFont) -> PdfResult<String> {
        use crate::objects::pdf_name::PdfName;
        self.document.edit_transaction(|doc| {
            let location = doc.page_location(page_index)?;
            let mut dict = PdfDict::default();
            for (key, value) in [
                ("Type", "Font"),
                ("Subtype", "Type1"),
                ("BaseFont", font.base_name()),
                ("Encoding", "WinAnsiEncoding"),
            ] {
                dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))?;
            }
            let resource = super::page_tree::PageTree::inherited_attribute(
                &doc.ctx,
                &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
                "Resources",
            )?;
            if let Some(resource) = resource {
                let resource = resource
                    .into_dict()
                    .ok_or(PdfError::PageResourcesIsNotDict)?;
                if let Some(fonts) = resource.get("Font") {
                    let fonts = doc.ctx.resolve(fonts)?.into_dict().ok_or_else(|| {
                        PdfError::ObjectError("font resources must be a dictionary".into())
                    })?;
                    let mut reused: Option<&String> = None;
                    for (name, value) in &fonts {
                        if doc.ctx.resolve(value)?.as_dict() == Some(&dict)
                            && reused.is_none_or(|previous| name < previous)
                        {
                            reused = Some(name);
                        }
                    }
                    if let Some(name) = reused {
                        return Ok(name.clone());
                    }
                }
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(dict))?;
            doc.resources()
                .register_existing(page_index, ResourceKind::Font, id)
        })
    }

    /// Decode PNG pixels and reuse RGB/RGBA embedding. No APNG, 16-bit or ICC conversion.
    pub fn embed_png_image(&self, page_index: u32, encoded: Vec<u8>) -> PdfResult<String> {
        let limits = self.document.ctx.limits();
        // Reject invalid pages before doing any potentially expensive native/codec work.
        self.document.page_location(page_index)?;
        if encoded.len() > limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("embedded PNG bytes".into()));
        }
        let png_error = |error: png::DecodingError| match error {
            png::DecodingError::LimitsExceeded => {
                PdfError::ResourceLimit("PNG decoder memory limit".into())
            }
            _ => PdfError::ObjectError("invalid PNG data".into()),
        };
        let mut decoder = png::Decoder::new_with_limits(
            std::io::Cursor::new(&encoded),
            png::Limits {
                bytes: limits.max_decoded_bytes,
            },
        );
        decoder.set_transformations(png::Transformations::EXPAND);
        decoder.set_ignore_text_chunk(true);
        let header = decoder.read_header_info().map_err(png_error)?;
        let (width, height) = (header.width, header.height);
        let count = (width as usize)
            .checked_mul(height as usize)
            .filter(|&n| n as u64 <= limits.max_image_pixels)
            .ok_or_else(|| PdfError::ResourceLimit("PNG pixel count".into()))?;
        let rgba_size = count
            .checked_mul(4)
            .filter(|&n| n <= limits.max_decoded_bytes && n <= limits.max_raw_stream_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("PNG pixel bytes".into()))?;
        if header.bit_depth == png::BitDepth::Sixteen {
            return Err(PdfError::UnsupportedFeature("16-bit PNG embedding".into()));
        }
        let mut reader = decoder.read_info().map_err(png_error)?;
        if reader.info().animation_control.is_some() || reader.info().icc_profile.is_some() {
            return Err(PdfError::UnsupportedFeature(
                "APNG/ICC PNG embedding".into(),
            ));
        }
        let size = reader
            .output_buffer_size()
            .filter(|&n| n <= rgba_size)
            .ok_or_else(|| PdfError::ResourceLimit("PNG output buffer size".into()))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("PNG output allocation".into()))?;
        pixels.resize(size, 0);
        let output = reader.next_frame(&mut pixels).map_err(png_error)?;
        reader.finish().map_err(png_error)?;
        if output.bit_depth != png::BitDepth::Eight {
            return Err(PdfError::UnsupportedFeature("PNG sample depth".into()));
        }
        pixels.truncate(output.buffer_size());
        match output.color_type {
            png::ColorType::Rgb => self.embed_rgb_image(page_index, width, height, pixels),
            png::ColorType::Rgba => self.embed_rgba_image(page_index, width, height, pixels),
            png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha => {
                let alpha = output.color_type == png::ColorType::GrayscaleAlpha;
                let mut expanded = Vec::new();
                expanded
                    .try_reserve_exact(if alpha { rgba_size } else { count * 3 })
                    .map_err(|_| {
                        PdfError::ResourceLimit("PNG grayscale expansion allocation".into())
                    })?;
                for sample in pixels.chunks_exact(if alpha { 2 } else { 1 }) {
                    expanded.extend_from_slice(&[sample[0]; 3]);
                    if alpha {
                        expanded.push(sample[1]);
                    }
                }
                if alpha {
                    self.embed_rgba_image(page_index, width, height, expanded)
                } else {
                    self.embed_rgb_image(page_index, width, height, expanded)
                }
            }
            _ => Err(PdfError::UnsupportedFeature("PNG output color type".into())),
        }
    }

    /// Embed JPEG bytes unchanged as DCTDecode. Header validation only, not full JPEG decoding.
    /// RGB/YCbCr and grayscale are supported; CMYK/YCCK require separate inversion policy.
    pub fn embed_jpeg_image(&self, page_index: u32, encoded: Vec<u8>) -> PdfResult<String> {
        use crate::objects::{pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream};
        let limits = self.document.ctx.limits();
        if encoded.len() > limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("embedded JPEG bytes".into()));
        }
        validate_jpeg_precision(&encoded)?;
        let header = mozjpeg::Decompress::new_mem(&encoded)
            .map_err(|_| PdfError::ObjectError("invalid JPEG header".into()))?;
        let (width, height) = header.size();
        if width == 0 || height == 0 {
            return Err(PdfError::ObjectError(
                "JPEG dimensions must be positive".into(),
            ));
        }
        let colorspace = match header.color_space() {
            mozjpeg::ColorSpace::JCS_GRAYSCALE => "DeviceGray",
            mozjpeg::ColorSpace::JCS_RGB | mozjpeg::ColorSpace::JCS_YCbCr => "DeviceRGB",
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "JPEG embedding color space".into(),
                ));
            }
        };
        let count = width
            .checked_mul(height)
            .filter(|&count| count as u64 <= limits.max_image_pixels)
            .ok_or_else(|| PdfError::ResourceLimit("embedded JPEG pixels".into()))?;
        count
            .checked_mul(4)
            .filter(|&n| n <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("embedded JPEG RGBA budget".into()))?;
        drop(header);
        self.document.edit_transaction(|doc| {
            doc.page_location(page_index)?;
            let mut dict = PdfDict::default();
            for (key, value) in [
                ("Type", "XObject"),
                ("Subtype", "Image"),
                ("ColorSpace", colorspace),
                ("Filter", "DCTDecode"),
            ] {
                dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))?;
            }
            for (key, value) in [
                ("Width", width),
                ("Height", height),
                ("BitsPerComponent", 8),
            ] {
                dict.try_insert(
                    key.into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value.to_string().as_bytes())?),
                )?;
            }
            let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, encoded)))?;
            doc.resources()
                .register_existing(page_index, ResourceKind::Image, id)
        })
    }

    /// Embed straight (not premultiplied) RGBA using a grayscale SMask.
    /// The interpreter supports matching-size 8-bit DeviceGray masks.
    pub fn embed_rgba_image(
        &self,
        page_index: u32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> PdfResult<String> {
        use crate::objects::{pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream};
        let limits = self.document.ctx.limits();
        if width == 0 || height == 0 {
            return Err(PdfError::ObjectError(
                "image dimensions must be positive".into(),
            ));
        }
        let count = (width as usize)
            .checked_mul(height as usize)
            .filter(|&count| count as u64 <= limits.max_image_pixels)
            .ok_or_else(|| PdfError::ResourceLimit("RGBA image dimensions/pixels".into()))?;
        let length = count
            .checked_mul(4)
            .filter(|&n| n <= limits.max_raw_stream_bytes && n <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("RGBA image bytes".into()))?;
        if pixels.len() != length {
            return Err(PdfError::ObjectError(
                "RGBA data length does not match dimensions".into(),
            ));
        }
        if self.document.effective_version()? < super::save_options::PdfVersion::V1_4 {
            return Err(PdfError::UnsupportedFeature(
                "RGBA soft masks require PDF 1.4".into(),
            ));
        }
        self.document.edit_transaction(|doc| {
            doc.page_location(page_index)?;
            let mut rgb = Vec::new();
            let mut alpha = Vec::new();
            rgb.try_reserve_exact(count * 3)
                .map_err(|_| PdfError::ResourceLimit("RGBA RGB allocation".into()))?;
            alpha
                .try_reserve_exact(count)
                .map_err(|_| PdfError::ResourceLimit("RGBA alpha allocation".into()))?;
            for pixel in pixels.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
                alpha.push(pixel[3]);
            }
            let mut mask = PdfDict::default();
            for (key, value) in [
                ("Type", "XObject"),
                ("Subtype", "Image"),
                ("ColorSpace", "DeviceGray"),
            ] {
                mask.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))?;
            }
            for (key, value) in [
                ("Width", width),
                ("Height", height),
                ("BitsPerComponent", 8),
            ] {
                mask.try_insert(
                    key.into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value.to_string().as_bytes())?),
                )?;
            }
            let mask_id =
                doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(mask, alpha)))?;
            let name = doc
                .resources()
                .embed_rgb_image(page_index, width, height, rgb)?;
            let resources = doc.get_page(page_index)?.resource()?;
            let id = resources
                .get("XObject")
                .and_then(PdfObject::as_dict)
                .and_then(|dict| dict.get(&name))
                .and_then(PdfObject::as_reference)
                .map(|reference| reference.id())
                .ok_or_else(|| PdfError::DocumentError("registered RGBA image missing".into()))?;
            let image = doc.resolve_object(id)?;
            let stream = image.as_stream().ok_or_else(|| {
                PdfError::DocumentError("registered RGBA image is not a stream".into())
            })?;
            let mut dict = stream.dict().clone();
            dict.try_insert(
                "SMask".into(),
                PdfObject::PdfReference(PdfReference::from_id(mask_id)),
            )?;
            let mut image_bytes = Vec::new();
            image_bytes
                .try_reserve_exact(stream.raw_data().len())
                .map_err(|_| PdfError::ResourceLimit("RGBA image update allocation".into()))?;
            image_bytes.extend_from_slice(stream.raw_data());
            doc.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, image_bytes)))?;
            Ok(name)
        })
    }

    /// Embed interleaved 8-bit RGB pixels. Alpha and encoded image files use separate APIs.
    pub fn embed_rgb_image(
        &self,
        page_index: u32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> PdfResult<String> {
        use crate::objects::{pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream};
        let limits = self.document.ctx.limits();
        let count = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| PdfError::ResourceLimit("image pixel count overflow".into()))?;
        if width == 0 || height == 0 {
            return Err(PdfError::ObjectError(
                "image dimensions must be positive".into(),
            ));
        }
        if count > limits.max_image_pixels {
            return Err(PdfError::ResourceLimit("image pixel count".into()));
        }
        let bytes = count
            .checked_mul(3)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| PdfError::ResourceLimit("image byte count overflow".into()))?;
        if bytes > limits.max_raw_stream_bytes || bytes > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("embedded RGB image bytes".into()));
        }
        if pixels.len() != bytes {
            return Err(PdfError::ObjectError(
                "RGB image data length does not match dimensions".into(),
            ));
        }
        self.document.edit_transaction(|doc| {
            // Resolve the target first so an invalid page cannot allocate image objects.
            doc.page_location(page_index)?;
            let mut dict = PdfDict::default();
            for (key, value) in [
                ("Type", "XObject"),
                ("Subtype", "Image"),
                ("ColorSpace", "DeviceRGB"),
            ] {
                dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))?;
            }
            for (key, value) in [
                ("Width", width),
                ("Height", height),
                ("BitsPerComponent", 8),
            ] {
                dict.try_insert(
                    key.into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value.to_string().as_bytes())?),
                )?;
            }
            let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, pixels)))?;
            doc.resources()
                .register_existing(page_index, ResourceKind::Image, id)
        })
    }

    /// Replace an existing page image binding without changing the old image or
    /// shared resource dictionaries. All uses of this name on the page change;
    /// this is not an individual Do edit or secure deletion.
    pub fn replace_image_handle(
        &self,
        page_index: u32,
        name: &str,
        handle: &super::identity::ObjectHandle,
    ) -> PdfResult<()> {
        let doc = self.document;
        let id = doc.validate_handle(handle)?;
        let validate_image = |object: &PdfObject| -> PdfResult<()> {
            let stream = object.as_stream().ok_or_else(|| {
                PdfError::ObjectError("image binding must reference a stream".into())
            })?;
            let subtype = stream
                .dict()
                .get("Subtype")
                .ok_or_else(|| PdfError::ObjectError("image subtype missing".into()))?;
            if doc
                .ctx
                .resolve_owned(subtype)?
                .as_name()
                .map(|name| name.name())
                != Some("Image")
            {
                return Err(PdfError::ObjectError(
                    "image binding requires Subtype Image".into(),
                ));
            }
            Ok(())
        };
        validate_image(&doc.resolve_object(id)?)?;
        let location = doc.page_location(page_index)?;
        let mut resources = super::page_tree::PageTree::inherited_attribute(
            &doc.ctx,
            &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
            "Resources",
        )?
        .ok_or(PdfError::PageResourcesIsNotDict)?
        .into_dict()
        .ok_or(PdfError::PageResourcesIsNotDict)?;
        let mut images =
            doc.ctx
                .resolve_owned(resources.get("XObject").ok_or_else(|| {
                    PdfError::PageResourceError("XObject resources missing".into())
                })?)?
                .into_dict()
                .ok_or_else(|| {
                    PdfError::PageResourceError("XObject resources must be dictionary".into())
                })?;
        let old = images
            .get(name)
            .ok_or_else(|| PdfError::PageResourceError("image resource name missing".into()))?;
        validate_image(&doc.ctx.resolve_owned(old)?)?;
        images.try_insert(
            name.into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        )?;
        resources.try_insert("XObject".into(), PdfObject::PdfDict(images))?;
        let mut page = location.page_dict;
        page.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        doc.edit_transaction(|doc| doc.update_object(location.page_id, PdfObject::PdfDict(page)))
    }

    /// Register a color-space value, reusing an exactly equal value in this
    /// page's resource scope. References inside the value must belong to this
    /// document; use import_resource_from for foreign object graphs.
    pub fn register_color_space(&self, page_index: u32, object: PdfObject) -> PdfResult<String> {
        self.document.edit_transaction(|doc| {
            let location = doc.page_location(page_index)?;
            let object = if object.as_reference().is_some() {
                doc.ctx.resolve_owned(&object)?
            } else {
                object
            };
            crate::color::colorspace::ColorSpace::try_new(&object, &doc.ctx)?;
            if let Some(resources) = super::page_tree::PageTree::inherited_attribute(
                &doc.ctx,
                &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
                "Resources",
            )? {
                let resources = resources
                    .into_dict()
                    .ok_or(PdfError::PageResourcesIsNotDict)?;
                if let Some(entries) = resources.get("ColorSpace") {
                    let entries = doc.ctx.resolve(entries)?.into_dict().ok_or_else(|| {
                        PdfError::ObjectError("color space resources must be a dictionary".into())
                    })?;
                    let mut reused: Option<&String> = None;
                    for (name, value) in &entries {
                        if doc.ctx.resolve(value)? == object
                            && reused.is_none_or(|previous| name < previous)
                        {
                            reused = Some(name);
                        }
                    }
                    if let Some(name) = reused {
                        return Ok(name.clone());
                    }
                }
            }
            let id = doc.try_insert_object(object)?;
            doc.resources()
                .register_existing(page_index, ResourceKind::ColorSpace, id)
        })
    }

    /// Register a scoped address, rejecting foreign documents before editing.
    pub fn register_handle(
        &self,
        page_index: u32,
        kind: ResourceKind,
        handle: &super::identity::ObjectHandle,
    ) -> PdfResult<String> {
        let id = self.document.validate_handle(handle)?;
        self.register_existing(page_index, kind, id)
    }

    /// Legacy unscoped ID entry point: the caller must ensure document ownership.
    /// Prefer `register_handle` when resources can originate in other documents.
    pub fn register_existing(
        &self,
        page_index: u32,
        kind: ResourceKind,
        id: ObjectId,
    ) -> PdfResult<String> {
        let doc = self.document;
        doc.edit_transaction(|doc| {
            let object = doc.resolve_object(id)?;
            let (category, prefix) = match kind {
                ResourceKind::ColorSpace => {
                    crate::color::colorspace::ColorSpace::try_new(&object, &doc.ctx)?;
                    ("ColorSpace", "CS")
                }
                ResourceKind::Form => {
                    crate::page::pdf_form::PdfForm::try_new(&object, &doc.ctx)?;
                    ("XObject", "Fm")
                }
                ResourceKind::Font => {
                    let dict = object.as_dict().ok_or_else(|| {
                        PdfError::ObjectError("font resource must be a dictionary".into())
                    })?;
                    let valid = dict
                        .get("Type")
                        .map(|value| doc.ctx.resolve(value))
                        .transpose()?
                        .is_some_and(|value| {
                            value.as_name().is_some_and(|name| name.bytes() == b"Font")
                        });
                    if !valid {
                        return Err(PdfError::ObjectError(
                            "font resource must have Type Font".into(),
                        ));
                    }
                    ("Font", "F")
                }
                ResourceKind::Image => {
                    let stream = object.as_stream().ok_or_else(|| {
                        PdfError::ObjectError("image resource must be a stream".into())
                    })?;
                    let valid = stream
                        .dict()
                        .get("Subtype")
                        .map(|value| doc.ctx.resolve(value))
                        .transpose()?
                        .is_some_and(|value| {
                            value.as_name().is_some_and(|name| name.bytes() == b"Image")
                        });
                    if !valid {
                        return Err(PdfError::ObjectError(
                            "image resource must have Subtype Image".into(),
                        ));
                    }
                    ("XObject", "Im")
                }
            };
            let location = doc.page_location(page_index)?;
            let mut resources = match super::page_tree::PageTree::inherited_attribute(
                &doc.ctx,
                &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
                "Resources",
            )? {
                Some(value) => value.into_dict().ok_or(PdfError::PageResourcesIsNotDict)?,
                None => PdfDict::default(),
            };
            let mut page = location.page_dict;
            let mut entries = match resources.get(category) {
                Some(value) => doc.ctx.resolve(value)?.into_dict().ok_or_else(|| {
                    PdfError::ObjectError("resource category must be a dictionary".into())
                })?,
                None => PdfDict::default(),
            };
            if let Some(name) = (&entries)
                .into_iter()
                .filter(|(_, value)| {
                    value
                        .as_reference()
                        .is_some_and(|reference| reference.id() == id)
                })
                .map(|(name, _)| name)
                .min()
            {
                return Ok(name.clone());
            }
            let count = (&entries).into_iter().count();
            if count >= doc.ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("page resource count".into()));
            }
            let mut index = 1usize;
            let name = loop {
                let candidate = format!("{prefix}{index}");
                if !entries.contains_key(&candidate) {
                    break candidate;
                }
                index = index
                    .checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("resource name exhausted".into()))?;
            };
            entries.try_insert(
                name.clone(),
                PdfObject::PdfReference(PdfReference::from_id(id)),
            )?;
            resources.try_insert(category.into(), PdfObject::PdfDict(entries))?;
            page.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
            doc.update_object(location.page_id, PdfObject::PdfDict(page))?;
            Ok(name)
        })
    }
}

fn validate_jpeg_precision(bytes: &[u8]) -> PdfResult<()> {
    let invalid = || PdfError::ObjectError("invalid JPEG marker header".into());
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(invalid());
    }
    let mut position = 2usize;
    while position < bytes.len() {
        if bytes[position] != 0xff {
            return Err(invalid());
        }
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes.get(position).ok_or_else(invalid)?;
        position += 1;
        if matches!(marker, 0xda | 0xd9 | 0x00) {
            return Err(invalid());
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let length_bytes = bytes.get(position..position + 2).ok_or_else(invalid)?;
        let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
        if length < 2 || length > bytes.len() - position {
            return Err(invalid());
        }
        if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            if length < 8 {
                return Err(invalid());
            }
            if !matches!(marker, 0xc0 | 0xc1 | 0xc2) || bytes[position + 2] != 8 {
                return Err(PdfError::UnsupportedFeature(
                    "JPEG embedding requires 8-bit sequential/progressive samples".into(),
                ));
            }
            return Ok(());
        }
        position += length;
    }
    Err(invalid())
}

//! WA-04/05 C ABI. Handles are module-local and never reused.
mod api;
mod content;
mod fonts;
mod interaction;
mod resources;
use api::{ApiError, ApiResult};
use pdf::{
    document::Document,
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use render::render_device::{RasterRegion, RgbaImage, SubstituteFont};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use zeroize::Zeroize;

const SAVE_LIMIT: usize = BUFFER_LIMIT;
const FONT_LIMIT: usize = 32 * 1024 * 1024;
const BUFFER_LIMIT: usize = 64 * 1024 * 1024;
fn limits() -> Limits {
    Limits {
        max_objects: 100_000,
        max_depth: 64,
        max_decoded_bytes: 16 * 1024 * 1024,
        max_operation_bytes: 64 * 1024 * 1024,
        max_image_pixels: 4_000_000,
        max_render_pixels: 4_000_000,
        max_render_bytes: 16_000_000,
        max_raw_stream_bytes: BUFFER_LIMIT,
        max_cached_stream_bytes: 16 * 1024 * 1024,
        max_content_operations: 100_000,
        max_text_chars: 100_000,
        max_path_segments: 100_000,
        max_token_bytes: 1024 * 1024,
        ..Limits::default()
    }
}
enum Resource {
    Input(Vec<u8>),
    Document(Rc<Document>, usize),
    Output(Vec<u8>),
    Image(RgbaImage),
}
impl Resource {
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Input(b) | Self::Output(b) => Some(b),
            Self::Image(i) => Some(&i.pixels),
            _ => None,
        }
    }
}
#[derive(Default)]
struct State {
    next: u32,
    resources: BTreeMap<u32, Resource>,
    error: String,
    error_code: u32,
    error_details: String,
    substitute_font: Option<SubstituteFont>,
    font_provider: Rc<render::font_provider::MemoryFontProvider>,
    font_epoch: u64,
    render_sessions: BTreeMap<u32, render::render_device::RenderSession>,
}
impl State {
    fn retained(&self) -> usize {
        self.resources
            .values()
            .filter_map(Resource::bytes)
            .map(<[u8]>::len)
            .sum::<usize>()
            + self
                .resources
                .values()
                .map(|r| match r {
                    Resource::Document(_, bytes) => *bytes,
                    _ => 0,
                })
                .sum::<usize>()
            + self
                .substitute_font
                .as_ref()
                .map_or(0, SubstituteFont::byte_len)
            + self.font_provider.byte_len()
    }
    fn insert(&mut self, resource: Resource) -> ApiResult<u32> {
        if self.resources.len() >= 8 {
            return Err(ApiError::resource("handle limit reached"));
        }
        if self
            .retained()
            .checked_add(match &resource {
                Resource::Document(_, bytes) => *bytes,
                _ => resource.bytes().map_or(0, <[u8]>::len),
            })
            .is_none_or(|n| n > BUFFER_LIMIT)
        {
            return Err(ApiError::resource("retained buffer limit exceeded"));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or("handle space exhausted; recreate module")?;
        self.resources.insert(self.next, resource);
        Ok(self.next)
    }
}
thread_local! { static STATE: RefCell<State> = {
    // Each Emscripten module owns an isolated Skia backend. Native consumers
    // and host-side tests keep their process-wide cache policy unchanged.
    #[cfg(target_os = "emscripten")]
    render::configure_isolated_web_caches();
    RefCell::new(State::default())
}; }
fn call(f: impl FnOnce(&mut State) -> ApiResult<u32>) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.error.clear();
        s.error_code = 0;
        s.error_details.clear();
        match f(&mut s) {
            Ok(n) => n,
            Err(e) => {
                s.error = e.message;
                s.error_code = e.code;
                s.error_details = e.details.map_or_else(String::new, |v| v.to_string());
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_abi_version() -> u32 {
    2
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_alloc(length: u32) -> u32 {
    call(|s| {
        let n = length as usize;
        if n == 0 || n > BUFFER_LIMIT || s.retained() + n > BUFFER_LIMIT {
            return Err(ApiError::resource("invalid input length or buffer limit"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(n)
            .map_err(|_| "input allocation failed")?;
        bytes.resize(n, 0);
        s.insert(Resource::Input(bytes))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_buffer_ptr(handle: u32) -> usize {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.error.clear();
        s.error_code = 0;
        s.error_details.clear();
        match s.resources.get(&handle).and_then(Resource::bytes) {
            Some(b) => b.as_ptr() as usize,
            None => {
                s.error = "invalid buffer handle".into();
                s.error_code = 8;
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_buffer_len(handle: u32) -> u32 {
    call(|s| {
        s.resources
            .get(&handle)
            .and_then(Resource::bytes)
            .map(|b| b.len() as u32)
            .ok_or_else(|| ApiError::handle("invalid buffer handle"))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_open(input: u32) -> u32 {
    call(|s| api::open(s, input, None, ParseMode::Strict))
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_page_count(document: u32) -> u32 {
    call(|s| match s.resources.get(&document) {
        Some(Resource::Document(d, _)) => Ok(d.try_page_count()? as u32),
        _ => Err(ApiError::handle("invalid document handle")),
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_render(document: u32, page: u32, dpi: f32) -> u32 {
    call(|s| render(s, document, page, dpi, None, None, 0))
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_render_page(
    document: u32,
    page: u32,
    dpi: f32,
    flags: u32,
    view_zoom: f32,
) -> u32 {
    call(|s| {
        render(
            s,
            document,
            page,
            dpi,
            render_options(flags, view_zoom)?,
            None,
            flags,
        )
    })
}
fn render_options(
    flags: u32,
    view_zoom: f32,
) -> ApiResult<Option<pdf::page::annotations::AnnotationRenderOptions>> {
    if flags > 31 || flags & 3 == 2 || !view_zoom.is_finite() || !(0.1..=4.).contains(&view_zoom) {
        return Err("invalid render options".into());
    }
    Ok(
        (flags & 1 != 0).then_some(pdf::page::annotations::AnnotationRenderOptions {
            include_widgets: flags & 2 != 0,
            view_zoom,
            view_rotation: (((flags >> 2) & 3) * 90) as u16,
            ..Default::default()
        }),
    )
}
/// Rasterize an integer pixel region without allocating the full-page surface.
#[unsafe(no_mangle)]
pub extern "C" fn rz_render_region(
    document: u32,
    page: u32,
    dpi: f32,
    flags: u32,
    view_zoom: f32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> u32 {
    call(|s| {
        if x > 32768 || y > 32768 || width == 0 || height == 0 || width > 1024 || height > 1024 {
            return Err("invalid render region".into());
        }
        render(
            s,
            document,
            page,
            dpi,
            render_options(flags, view_zoom)?,
            Some(RasterRegion {
                x,
                y,
                width,
                height,
            }),
            flags,
        )
    })
}

fn render(
    s: &mut State,
    document: u32,
    page: u32,
    dpi: f32,
    annotations: Option<pdf::page::annotations::AnnotationRenderOptions>,
    region: Option<RasterRegion>,
    flags: u32,
) -> ApiResult<u32> {
    let max_dpi = if region.is_some() {
        resources::MAX_REGION_DPI
    } else {
        resources::MAX_FULL_PAGE_DPI
    };
    if !dpi.is_finite() || !(1.0..=max_dpi).contains(&dpi) {
        return Err("DPI is outside the rendering mode's supported range".into());
    }
    if s.resources.len() >= 8 {
        return Err(ApiError::resource("release buffers before rendering"));
    }
    let Some(Resource::Document(d, _)) = s.resources.get(&document) else {
        return Err(ApiError::handle("invalid document handle"));
    };
    if page as usize >= d.page_count() {
        return Err("page index out of range".into());
    }
    let geometry = d.get_page(page)?.geometry()?;
    let (raster_width, raster_height) = render::raster_size::raster_dimensions(
        geometry.display_width,
        geometry.display_height,
        dpi,
        dpi,
    )?;
    let effective = d.resource_limits();
    let pixels = region.map_or(raster_width * raster_height, |r| {
        f64::from(r.width) * f64::from(r.height)
    });
    if pixels * 4. > BUFFER_LIMIT.saturating_sub(s.retained()) as f64 {
        return Err(pdf::error::PdfError::ResourceLimitExceeded {
            reason: "retained_bytes",
            stage: "raster_output",
            limit: BUFFER_LIMIT as u64,
            actual: s.retained().saturating_add((pixels * 4.) as usize) as u64,
        }
        .into());
    }
    for (reason, actual, limit) in [
        ("render_pixels", pixels, effective.max_render_pixels),
        (
            "render_bytes",
            pixels * 4.,
            effective.max_render_bytes as u64,
        ),
    ] {
        if actual > limit as f64 {
            return Err(pdf::error::PdfError::ResourceLimitExceeded {
                reason,
                stage: "raster_output",
                limit,
                actual: actual as u64,
            }
            .into());
        }
    }
    if let Some(region) = region {
        let (width, height) = (raster_width, raster_height);
        if width > 32768. || height > 32768. {
            return Err(pdf::error::PdfError::ResourceLimitExceeded {
                reason: "raster_side",
                stage: "raster_output",
                limit: 32768,
                actual: width.max(height) as u64,
            }
            .into());
        }
        if f64::from(region.x + region.width) > width
            || f64::from(region.y + region.height) > height
        {
            return Err("render region exceeds page".into());
        }
    }
    // Keep the module's optional backend cache bounded even with several
    // open documents: only the currently rendered document retains resources.
    for (handle, session) in &s.render_sessions {
        if *handle != document {
            session.clear();
        }
    }
    let session = s
        .render_sessions
        .entry(document)
        .or_insert_with(|| render::render_device::RenderSession::new(d.resource_limits()));
    let mut renderer = session
        .renderer(d, s.font_epoch, dpi, dpi)
        .map_err(ApiError::from)?;
    renderer.set_region(region)?;
    if let Some(program) = &s.substitute_font {
        renderer.set_substitute_program(program);
    }
    renderer.set_font_provider(s.font_provider.clone());
    // Interpretation is document/revision scoped; DPI and image policy enter
    // cache identity, while tile rectangles affect only replay. NoZoom/NoRotate annotation inputs remain part of the key.
    let prepared = d
        .prepare_page_for_render_with_policy(
            page,
            annotations,
            dpi,
            if flags & 16 != 0 {
                pdf::document::prepared_page::ImageDecodePolicy::TargetResolution
            } else {
                pdf::document::prepared_page::ImageDecodePolicy::Budgeted
            },
        )
        .map_err(ApiError::from)?;
    prepared.replay(d, &mut renderer).map_err(ApiError::from)?;
    s.insert(Resource::Image(renderer.to_rgba().map_err(ApiError::from)?))
}

/// Register one explicitly supplied fallback program; input=0 clears it.
#[unsafe(no_mangle)]
pub extern "C" fn rz_set_substitute_font(input: u32, index: u32) -> u32 {
    call(|s| {
        if input == 0 {
            let epoch = s
                .font_epoch
                .checked_add(1)
                .ok_or_else(|| ApiError::resource("font epoch exhausted"))?;
            s.font_epoch = epoch;
            s.substitute_font = None;
            return Ok(1);
        }
        let Some(Resource::Input(bytes)) = s.resources.get(&input) else {
            return Err(ApiError::handle("invalid font input handle"));
        };
        if bytes.is_empty() || bytes.len() > FONT_LIMIT {
            return Err(ApiError::resource(
                "font input must contain 1 byte to 32 MiB",
            ));
        }
        let previous = s
            .substitute_font
            .as_ref()
            .map_or(0, SubstituteFont::byte_len);
        if s.retained() - previous + bytes.len() > BUFFER_LIMIT {
            return Err(ApiError::resource("font buffer limit exceeded"));
        }
        let program = SubstituteFont::from_bytes(bytes, index as usize).map_err(ApiError::from)?;
        let epoch = s
            .font_epoch
            .checked_add(1)
            .ok_or_else(|| ApiError::resource("font epoch exhausted"))?;
        s.font_epoch = epoch;
        s.substitute_font = Some(program);
        Ok(1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rz_image_width(image: u32) -> u32 {
    call(|s| match s.resources.get(&image) {
        Some(Resource::Image(i)) => Ok(i.width),
        _ => Err(ApiError::handle("invalid image handle")),
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_image_height(image: u32) -> u32 {
    call(|s| match s.resources.get(&image) {
        Some(Resource::Image(i)) => Ok(i.height),
        _ => Err(ApiError::handle("invalid image handle")),
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_release(handle: u32) -> u32 {
    call(|s| {
        let mut resource = s
            .resources
            .remove(&handle)
            .ok_or_else(|| ApiError::handle("invalid or released handle"))?;
        s.render_sessions.remove(&handle);
        if let Resource::Input(bytes) = &mut resource {
            bytes.zeroize();
        }
        Ok(1)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_error_ptr() -> usize {
    STATE.with(|s| s.borrow().error.as_ptr() as usize)
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_error_len() -> u32 {
    STATE.with(|s| s.borrow().error.len() as u32)
}

/// Borrowed UTF-8 JSON metadata; read before the next mutating ABI call.
#[unsafe(no_mangle)]
pub extern "C" fn rz_error_details_ptr() -> usize {
    STATE.with(|s| s.borrow().error_details.as_ptr() as usize)
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_error_details_len() -> u32 {
    STATE.with(|s| s.borrow().error_details.len() as u32)
}

/// Free tracked resources without resetting the monotonically increasing handle ID.
#[unsafe(no_mangle)]
pub extern "C" fn rz_dispose() -> u32 {
    call(|s| {
        for resource in s.resources.values_mut() {
            if let Resource::Input(bytes) = resource {
                bytes.zeroize();
            }
        }
        s.resources.clear();
        s.render_sessions.clear();
        s.font_provider = Default::default();
        s.substitute_font = None;
        #[cfg(target_os = "emscripten")]
        render::purge_isolated_web_caches();
        Ok(1)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_handle_count() -> u32 {
    STATE.with(|s| s.borrow().resources.len() as u32)
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_retained_bytes() -> u32 {
    STATE.with(|s| s.borrow().retained() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_rotation_flags_are_explicit_and_bounded() {
        for turns in 0..4 {
            let options = render_options(3 | (turns << 2), 1.).unwrap().unwrap();
            assert_eq!(options.view_rotation, (turns * 90) as u16);
            assert!(options.include_widgets);
            assert!(render_options(turns << 2, 1.).unwrap().is_none());
            assert!(render_options(2 | (turns << 2), 1.).is_err());
        }
        assert!(render_options(16, 1.).unwrap().is_none());
        assert!(render_options(32, 1.).is_err());
        assert_eq!(render_options(31, 1.).unwrap().unwrap().view_rotation, 270);
    }

    #[test]
    fn stale_handles_and_wrong_types_are_rejected() {
        let h = rz_alloc(10);
        assert_ne!(h, 0);
        assert_eq!(rz_page_count(h), 0);
        assert_eq!(rz_release(h), 1);
        assert_eq!(rz_release(h), 0);
        let next = rz_alloc(10);
        assert!(next > h);
        assert_eq!(rz_buffer_len(h), 0);
        assert_eq!(rz_release(next), 1);
    }
    #[test]
    fn font_registration_validates_and_preserves_previous_font_on_failure() {
        let good = STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(
                    include_bytes!("../../pdf/tests/fixtures/v07/geometry.ttf").to_vec(),
                ))
                .unwrap()
        });
        assert_eq!(rz_set_substitute_font(good, 0), 1);
        assert_eq!(rz_release(good), 1);
        let previous = STATE.with(|s| s.borrow().substitute_font.as_ref().unwrap().byte_len());
        let bad = rz_alloc(16);
        assert_eq!(rz_set_substitute_font(bad, 0), 0);
        assert_eq!(
            STATE.with(|s| s.borrow().substitute_font.as_ref().unwrap().byte_len()),
            previous
        );
        assert_eq!(rz_release(bad), 1);
        assert_eq!(rz_set_substitute_font(0, 0), 1);
        assert!(STATE.with(|s| s.borrow().substitute_font.is_none()));
    }

    #[test]
    fn disposal_releases_tracked_resources_and_never_reuses_old_handles() {
        rz_dispose();
        let input = rz_alloc(16);
        assert_eq!(rz_handle_count(), 1);
        assert_eq!(rz_retained_bytes(), 16);
        assert_eq!(rz_dispose(), 1);
        assert_eq!(rz_dispose(), 1);
        assert_eq!(rz_handle_count(), 0);
        assert_eq!(rz_retained_bytes(), 0);
        let next = rz_alloc(16);
        assert!(next > input);
        assert_eq!(rz_buffer_len(input), 0);
        assert_eq!(rz_release(next), 1);
    }

    #[test]
    fn region_api_bounds_large_pages_output_ownership_and_failure_release() {
        rz_dispose();
        let d = Document::new().unwrap();
        d.add_page_with_builder(
            0,
            pdf::page::page_builder::PageBuilder::new()
                .size(10000., 10000.)
                .content(b"1 0 0 rg 0 0 10000 10000 re f".to_vec()),
        )
        .unwrap();
        let bytes = d.save_to_bytes().unwrap();
        let input = STATE.with(|s| s.borrow_mut().insert(Resource::Input(bytes)).unwrap());
        let doc = rz_open(input);
        assert_ne!(doc, 0);
        assert_eq!(rz_release(input), 1);
        let baseline = rz_retained_bytes();
        assert_eq!(rz_render_page(doc, 0, 72., 3, 1.), 0);
        assert_eq!(api::rz_error_code(), 7);
        for (x, y, w, h) in [
            (0, 0, 0, 1),
            (0, 0, 1025, 1),
            (9999, 0, 64, 1),
            (0, 9999, 1, 64),
            (u32::MAX, 0, 1, 1),
        ] {
            assert_eq!(rz_render_region(doc, 0, 72., 3, 1., x, y, w, h), 0);
            assert_eq!(api::rz_error_code(), 9);
            assert_eq!(rz_handle_count(), 1);
            assert_eq!(rz_retained_bytes(), baseline);
        }
        let image = rz_render_region(doc, 0, 72., 3, 1., 9000, 8000, 64, 32);
        assert_ne!(image, 0);
        assert_eq!(rz_image_width(image), 64);
        assert_eq!(rz_image_height(image), 32);
        STATE.with(|s| {
            let s = s.borrow();
            let Resource::Image(image) = s.resources.get(&image).unwrap() else {
                panic!("image")
            };
            assert_eq!(image.pixels.len(), 8192);
            assert!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [255, 0, 0, 255])
            );
        });
        assert_eq!(rz_release(image), 1);
        assert_eq!(rz_retained_bytes(), baseline);
        assert_eq!(rz_release(doc), 1);
        assert_eq!(rz_handle_count(), 0);
        assert_eq!(rz_retained_bytes(), 0);
    }

    #[test]
    fn pdf_larger_than_eight_mib_opens_renders_and_saves_within_buffer_budget() {
        let mut bytes = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let bodies = [
            "<< /Type /Catalog /Pages 2 0 R /RzPadding 5 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 20 20] /Contents 4 0 R >>",
            "<< /Length 23 >>\nstream\n1 0 0 rg 0 0 20 20 re f\nendstream",
        ];
        let mut offsets = vec![0];
        for (index, body) in bodies.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
        }
        offsets.push(bytes.len());
        bytes.extend_from_slice(
            format!("5 0 obj\n<< /Length {} >>\nstream\n", 9 * 1024 * 1024).as_bytes(),
        );
        bytes.resize(bytes.len() + 9 * 1024 * 1024, b'X');
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        let xref = bytes.len();
        bytes.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
        for offset in &offsets[1..] {
            bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        let input = rz_alloc(bytes.len() as u32);
        assert_ne!(input, 0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let Resource::Input(data) = s.resources.get_mut(&input).unwrap() else {
                panic!("input")
            };
            data.copy_from_slice(&bytes);
        });
        let document = rz_open(input);
        assert_ne!(document, 0, "{}", STATE.with(|s| s.borrow().error.clone()));
        assert_eq!(rz_release(input), 1);
        let image = rz_render(document, 0, 72.);
        assert_ne!(image, 0);
        assert_eq!(rz_release(image), 1);
        let saved = interaction::rz_save(document, SAVE_LIMIT as u32);
        assert_ne!(saved, 0, "{}", STATE.with(|s| s.borrow().error.clone()));
        assert!(rz_buffer_len(saved) > 8 * 1024 * 1024);
        assert_eq!(rz_release(saved), 1);
        assert_eq!(rz_release(document), 1);
        assert_eq!(rz_retained_bytes(), 0);
        // Input allocation remains governed by the aggregate buffer budget.
        let large = rz_alloc(33 * 1024 * 1024);
        assert_ne!(large, 0);
        assert_eq!(rz_open(large), 0);
        assert_eq!(api::rz_error_code(), 7);
        assert_eq!(rz_set_substitute_font(large, 0), 0);
        assert_eq!(api::rz_error_code(), 7);
        assert_eq!(rz_release(large), 1);
    }
    #[test]
    fn input_and_render_parameters_are_bounded() {
        assert_eq!(rz_alloc(0), 0);
        assert_eq!(rz_alloc(BUFFER_LIMIT as u32 + 1), 0);
        let oversized = rz_alloc(8 * 1024 * 1024 + 1);
        assert_ne!(oversized, 0);
        assert_eq!(rz_open(oversized), 0);
        assert_eq!(rz_release(oversized), 1);
        assert_eq!(rz_render(0, 0, f32::NAN), 0);
        assert_eq!(rz_render(0, 0, 289.), 0);
    }
}

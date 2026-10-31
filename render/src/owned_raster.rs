//! Transfer a completed RGBA allocation to an immutable Skia pixel reference.
//! The release callback owns the buffer, including when image creation fails.
//! skia-safe's safe Bitmap installer cannot attach a release callback; keep the
//! generated installPixels binding and its ownership contract isolated here.
use pdf::error::{PdfError, PdfResult};
use skia_safe::{AlphaType, Bitmap, ColorType, Image, ImageInfo, wrapper::ValueWrapper};
use std::ffi::c_void;

struct Pixels {
    bytes: Box<[u8]>,
    #[cfg(test)]
    dropped: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

#[cfg(test)]
impl Drop for Pixels {
    fn drop(&mut self) {
        if let Some(counter) = &self.dropped {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

unsafe extern "C" fn release_pixels(_address: *mut c_void, context: *mut c_void) {
    // SAFETY: install() transfers exactly one Box<Pixels> to Skia. Its pixel
    // reference invokes this callback once, on failure or after its final owner
    // is released. Pixels contains only thread-safe owned bytes (and, in tests,
    // an atomic drop counter), and its destructor cannot unwind.
    drop(unsafe { Box::from_raw(context.cast::<Pixels>()) });
}

fn install(info: &ImageInfo, row_bytes: usize, mut pixels: Box<Pixels>) -> Option<Image> {
    if info.width() <= 0
        || info.height() <= 0
        || row_bytes < info.min_row_bytes()
        || row_bytes
            .checked_mul(info.height() as usize - 1)
            .and_then(|n| n.checked_add(info.min_row_bytes()))
            .is_none_or(|n| n > pixels.bytes.len())
    {
        return None;
    }
    let address = pixels.bytes.as_mut_ptr().cast::<c_void>();
    let context = Box::into_raw(pixels).cast::<c_void>();
    let mut bitmap = Bitmap::new();
    // SAFETY: the buffer is fully initialized, owned exclusively by context,
    // and remains alive until Skia's release callback. installPixels consumes
    // the callback even on failure, so Rust must not free context afterward.
    // The public caller checks its exact size/stride. The bitmap is made
    // immutable before sharing; no mutable Rust reference escapes this call.
    let installed = unsafe {
        bitmap.inner_mut().installPixels(
            info.inner(),
            address,
            row_bytes,
            Some(release_pixels),
            context,
        )
    };
    if !installed {
        return None;
    }
    bitmap.set_immutable();
    skia_safe::images::raster_from_bitmap(&bitmap)
}

pub(crate) fn from_rgba(width: i32, height: i32, bytes: Vec<u8>) -> PdfResult<Image> {
    let size = usize::try_from(width)
        .ok()
        .filter(|w| *w > 0)
        .zip(usize::try_from(height).ok().filter(|h| *h > 0))
        .and_then(|(w, h)| w.checked_mul(h))
        .and_then(|n| n.checked_mul(4));
    if size != Some(bytes.len()) {
        return Err(PdfError::ObjectError(
            "raster RGBA buffer length mismatch".into(),
        ));
    }
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    from_pixels(&info, bytes)
}

/// Transfer tightly packed native RGBA/BGRA bytes without a second Skia copy.
/// Keep the caller's alpha convention (group correction produces premultiplied
/// N32, whereas decoded images and ICC conversion produce straight RGBA).
pub(crate) fn from_pixels(info: &ImageInfo, bytes: Vec<u8>) -> PdfResult<Image> {
    if !matches!(info.color_type(), ColorType::RGBA8888 | ColorType::BGRA8888)
        || info.width() <= 0
        || info.height() <= 0
        || info.min_row_bytes().checked_mul(info.height() as usize) != Some(bytes.len())
    {
        return Err(PdfError::ObjectError("owned raster layout mismatch".into()));
    }
    let pixels = Box::new(Pixels {
        bytes: bytes.into_boxed_slice(),
        #[cfg(test)]
        dropped: None,
    });
    install(info, info.min_row_bytes(), pixels)
        .ok_or_else(|| PdfError::ResourceLimit("Skia rejected image pixel buffer".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn owner(counter: &Arc<AtomicUsize>) -> Box<Pixels> {
        Box::new(Pixels {
            bytes: [17, 39, 81, 127].repeat(4).into_boxed_slice(),
            dropped: Some(counter.clone()),
        })
    }

    #[test]
    fn image_shares_pixels_and_releases_once_after_last_clone_and_mipmap() {
        let counter = Arc::new(AtomicUsize::new(0));
        let pixels = owner(&counter);
        let address = pixels.bytes.as_ptr();
        let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Unpremul, None);
        let image = install(&info, 8, pixels).unwrap();
        assert_eq!(
            image.peek_pixels().unwrap().bytes().unwrap().as_ptr(),
            address
        );
        let clone = image.clone();
        let mipmaps = image.with_default_mipmaps().unwrap();
        drop(image);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        drop(clone);
        // Skia copies the base pixels when constructing mipmaps. That image
        // must remain valid after the transferred source allocation is freed.
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        assert_eq!(
            &mipmaps.peek_pixels().unwrap().bytes().unwrap()[..4],
            &[17, 39, 81, 127]
        );
        drop(mipmaps);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        let bytes = vec![23; 16];
        let address = bytes.as_ptr();
        let direct = from_rgba(2, 2, bytes).unwrap();
        assert_eq!(
            direct.peek_pixels().unwrap().bytes().unwrap().as_ptr(),
            address
        );
    }

    #[test]
    fn native_rejection_releases_transferred_owner() {
        let counter = Arc::new(AtomicUsize::new(0));
        // Size and stride are valid, but RGBA cannot use unknown alpha.
        // This reaches Skia's rejection path after ownership is transferred.
        let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Unknown, None);
        assert!(install(&info, 8, owner(&counter)).is_none());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn rejected_layout_releases_owner_and_bad_lengths_are_errors() {
        let counter = Arc::new(AtomicUsize::new(0));
        let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Unpremul, None);
        assert!(install(&info, 1, owner(&counter)).is_none());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        for (w, h, n) in [(2, 2, 15), (2, 2, 17), (0, 2, 0), (-1, 2, 0)] {
            assert!(from_rgba(w, h, vec![0; n]).is_err());
        }
    }
}

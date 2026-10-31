//! Bundled jbig2dec 0.20 with every native allocation charged to the caller's
//! OperationBudget. ABI: vendor/jbig2dec/jbig2.h (upstream recorded alongside it).
use super::jbig2::{error, limit};
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::PdfResult,
};
use std::{
    alloc::{Layout, alloc, dealloc},
    cell::Cell,
    ffi::{c_char, c_int, c_void},
    ptr,
};

#[repr(C)]
struct Callbacks {
    alloc: unsafe extern "C" fn(*mut Callbacks, usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut Callbacks, *mut c_void),
    realloc: unsafe extern "C" fn(*mut Callbacks, *mut c_void, usize) -> *mut c_void,
}
#[repr(C)]
struct Allocator {
    callbacks: Callbacks,
    budget: OperationBudget,
    failed: Cell<bool>,
    required: Cell<usize>,
}
#[repr(C, align(16))]
struct Header {
    lease: MemoryLease,
    size: usize,
}
unsafe extern "C" fn allocate(a: *mut Callbacks, size: usize) -> *mut c_void {
    // SAFETY: the decoder receives a pointer to the first field of a live Allocator.
    let a = unsafe { &*(a as *const Allocator) };
    let layout = size
        .checked_add(size_of::<Header>())
        .and_then(|n| Layout::from_size_align(n, align_of::<Header>()).ok());
    let Some(layout) = layout else {
        a.failed.set(true);
        return ptr::null_mut();
    };
    let Ok(lease) = a.budget.reserve(layout.size()) else {
        a.failed.set(true);
        a.required.set(
            a.required
                .get()
                .max(a.budget.live_bytes().saturating_add(layout.size())),
        );
        return ptr::null_mut();
    };
    // SAFETY: valid nonzero layout; the header preserves ownership until free.
    let block = unsafe { alloc(layout) };
    if block.is_null() {
        a.failed.set(true);
        return ptr::null_mut();
    }
    unsafe {
        (block as *mut Header).write(Header { lease, size });
        block.add(size_of::<Header>()).cast()
    }
}
unsafe extern "C" fn free(_a: *mut Callbacks, p: *mut c_void) {
    if p.is_null() {
        return;
    }
    // SAFETY: only allocations returned by allocate are passed by jbig2dec.
    unsafe {
        let header = (p as *mut u8).sub(size_of::<Header>()) as *mut Header;
        let layout = Layout::from_size_align_unchecked(
            (*header).size + size_of::<Header>(),
            align_of::<Header>(),
        );
        ptr::drop_in_place(header);
        dealloc(header.cast(), layout);
    }
}
unsafe extern "C" fn reallocate(a: *mut Callbacks, p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return unsafe { allocate(a, size) };
    }
    if size == 0 {
        unsafe { free(a, p) };
        return ptr::null_mut();
    }
    // Reserve the replacement while the old buffer is still live. A failure
    // leaves the original buffer intact, as required by realloc's contract.
    let next = unsafe { allocate(a, size) };
    if !next.is_null() {
        unsafe {
            let header = (p as *mut u8).sub(size_of::<Header>()) as *const Header;
            ptr::copy_nonoverlapping(p.cast::<u8>(), next.cast::<u8>(), size.min((*header).size));
            free(a, p);
        }
    }
    next
}
#[repr(C)]
struct Image {
    width: u32,
    height: u32,
    stride: u32,
    data: *mut u8,
    refcount: c_int,
}
unsafe extern "C" {
    fn jbig2_ctx_new_imp(
        a: *mut Callbacks,
        options: c_int,
        globals: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, u32),
        data: *mut c_void,
        major: c_int,
        minor: c_int,
    ) -> *mut c_void;
    fn jbig2_ctx_free(ctx: *mut c_void) -> *mut Callbacks;
    fn jbig2_make_global_ctx(ctx: *mut c_void) -> *mut c_void;
    fn jbig2_data_in(ctx: *mut c_void, data: *const u8, len: usize) -> c_int;
    fn jbig2_complete_page(ctx: *mut c_void) -> c_int;
    fn jbig2_page_out(ctx: *mut c_void) -> *mut Image;
    fn jbig2_release_page(ctx: *mut c_void, image: *mut Image);
}
struct Context(*mut c_void);
impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            jbig2_ctx_free(self.0);
        }
    }
}
struct Page<'a> {
    image: *mut Image,
    ctx: &'a Context,
}
impl Drop for Page<'_> {
    fn drop(&mut self) {
        unsafe {
            jbig2_release_page(self.ctx.0, self.image);
        }
    }
}
unsafe extern "C" fn message(
    data: *mut c_void,
    _msg: *const c_char,
    severity: c_int,
    _segment: u32,
) {
    if severity == 3 {
        unsafe { &*(data as *const Cell<bool>) }.set(true);
    }
}
pub(super) fn decode(
    input: &[u8],
    globals: Option<&[u8]>,
    width: u32,
    height: u32,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    decode_inner(input, globals, width, height, budget, false)
}
pub(super) fn decode_preview(
    input: &[u8],
    globals: Option<&[u8]>,
    width: u32,
    height: u32,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    decode_inner(input, globals, width, height, budget, true)
}
fn decode_inner(
    input: &[u8],
    globals: Option<&[u8]>,
    width: u32,
    height: u32,
    budget: &OperationBudget,
    structured: bool,
) -> PdfResult<BudgetedBytes> {
    let allocator = Allocator {
        callbacks: Callbacks {
            alloc: allocate,
            free,
            realloc: reallocate,
        },
        budget: budget.clone(),
        failed: Cell::new(false),
        required: Cell::new(0),
    };
    let fatal = Cell::new(false);
    let failure = || {
        if allocator.failed.get() {
            if structured && allocator.required.get() > 0 {
                super::bilevel_preview::limit(
                    "image_operation_bytes",
                    budget.maximum_bytes() as u64,
                    allocator.required.get() as u64,
                )
            } else {
                limit("native allocation budget")
            }
        } else {
            error("invalid symbol/region data")
        }
    };
    let new_context = |global| -> PdfResult<Context> {
        let ctx = unsafe {
            jbig2_ctx_new_imp(
                (&allocator.callbacks as *const Callbacks).cast_mut(),
                1,
                global,
                message,
                (&fatal as *const Cell<bool>).cast_mut().cast(),
                0,
                20,
            )
        };
        if ctx.is_null() {
            Err(failure())
        } else {
            Ok(Context(ctx))
        }
    };
    // Drop page/context before Globals and before their allocator/callback data.
    let global = if let Some(bytes) = globals {
        let ctx = new_context(ptr::null_mut())?;
        if unsafe { jbig2_data_in(ctx.0, bytes.as_ptr(), bytes.len()) } < 0 || fatal.get() {
            return Err(failure());
        }
        Some(ctx)
    } else {
        None
    };
    let global_ptr = global
        .as_ref()
        .map_or(ptr::null_mut(), |g| unsafe { jbig2_make_global_ctx(g.0) });
    let ctx = new_context(global_ptr)?;
    if unsafe { jbig2_data_in(ctx.0, input.as_ptr(), input.len()) } < 0 || fatal.get() {
        return Err(failure());
    }
    if unsafe { jbig2_complete_page(ctx.0) } < 0 || fatal.get() {
        return Err(failure());
    }
    let image = unsafe { jbig2_page_out(ctx.0) };
    if image.is_null() {
        return Err(failure());
    }
    let page = Page { image, ctx: &ctx };
    let image = unsafe { &*page.image };
    let row = (width as usize).div_ceil(8);
    if image.width != width
        || image.height != height
        || (image.stride as usize) < row
        || image.data.is_null()
    {
        return Err(error("native output shape mismatch"));
    }
    let length = row
        .checked_mul(height as usize)
        .ok_or_else(|| limit("output size"))?;
    let lease = budget.reserve(length).map_err(|err| {
        if structured {
            super::bilevel_preview::limit(
                "image_operation_bytes",
                budget.maximum_bytes() as u64,
                budget.live_bytes().saturating_add(length) as u64,
            )
        } else {
            err
        }
    })?;
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|_| limit("output allocation"))?;
    for y in 0..height as usize {
        // jbig2dec stores black as 1; PDF JBIG2Decode samples use white as 1.
        let source =
            unsafe { std::slice::from_raw_parts(image.data.add(y * image.stride as usize), row) };
        data.extend(source.iter().map(|b| !b));
        if !width.is_multiple_of(8) {
            *data.last_mut().unwrap() &= 0xff << (8 - width % 8);
        }
    }
    BudgetedBytes::from_reserved_vec(data, lease)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_allocator_preserves_realloc_failure_and_releases_all_leases() {
        let budget = OperationBudget::new(256);
        let mut a = Allocator {
            callbacks: Callbacks {
                alloc: allocate,
                free,
                realloc: reallocate,
            },
            budget: budget.clone(),
            failed: Cell::new(false),
            required: Cell::new(0),
        };
        unsafe {
            let p = allocate(&mut a.callbacks, 32);
            assert!(!p.is_null());
            *(p as *mut u8) = 42;
            assert!(reallocate(&mut a.callbacks, p, 256).is_null());
            assert_eq!(*(p as *const u8), 42);
            assert!(budget.live_bytes() > 32);
            free(&mut a.callbacks, p);
            assert_eq!(budget.live_bytes(), 0);
        }
    }
}

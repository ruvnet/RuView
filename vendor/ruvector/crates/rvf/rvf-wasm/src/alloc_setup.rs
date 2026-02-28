//! Global allocator for WASM heap allocation.
//!
//! Uses dlmalloc as the global allocator, enabling Vec, String, etc.
//! Exposes rvf_alloc/rvf_free for JS interop memory management.

extern crate alloc;

use dlmalloc::GlobalDlmalloc;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

/// Allocate `size` bytes of memory, returning a pointer.
/// Returns 0 on failure.
#[no_mangle]
pub extern "C" fn rvf_alloc(size: i32) -> i32 {
    if size <= 0 {
        return 0;
    }
    let layout = match core::alloc::Layout::from_size_align(size as usize, 8) {
        Ok(l) => l,
        Err(_) => return 0,
    };
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        0
    } else {
        ptr as i32
    }
}

/// Free memory previously allocated by `rvf_alloc`.
#[no_mangle]
pub extern "C" fn rvf_free(ptr: i32, size: i32) {
    if ptr == 0 || size <= 0 {
        return;
    }
    let layout = match core::alloc::Layout::from_size_align(size as usize, 8) {
        Ok(l) => l,
        Err(_) => return,
    };
    unsafe {
        alloc::alloc::dealloc(ptr as *mut u8, layout);
    }
}

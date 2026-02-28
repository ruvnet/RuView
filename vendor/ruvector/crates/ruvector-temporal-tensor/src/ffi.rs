//! WASM/C FFI interface with handle-based resource management.
//!
//! Exports `extern "C"` functions for:
//! - Compressor lifecycle (`ttc_create`, `ttc_free`, `ttc_touch`, `ttc_set_access`)
//! - Frame compression (`ttc_push_frame`, `ttc_flush`)
//! - Segment decoding (`ttc_decode_segment`)
//! - Memory management (`ttc_alloc`, `ttc_dealloc`)

use crate::compressor::TemporalTensorCompressor;
use crate::segment;
use crate::tier_policy::TierPolicy;

static mut STORE: Option<Vec<Option<TemporalTensorCompressor>>> = None;

fn store_init() {
    unsafe {
        if STORE.is_none() {
            STORE = Some(Vec::new());
        }
    }
}

fn with_store<F, R>(f: F) -> R
where
    F: FnOnce(&mut Vec<Option<TemporalTensorCompressor>>) -> R,
{
    store_init();
    unsafe { f(STORE.as_mut().unwrap()) }
}

fn with_compressor<F>(handle: u32, f: F)
where
    F: FnOnce(&mut TemporalTensorCompressor),
{
    with_store(|store| {
        let idx = handle as usize;
        if idx < store.len() {
            if let Some(comp) = store[idx].as_mut() {
                f(comp);
            }
        }
    });
}

/// Create a new compressor. Returns handle via out_handle.
#[no_mangle]
pub extern "C" fn ttc_create(len: u32, now_ts: u32, out_handle: *mut u32) {
    let policy = TierPolicy::default();
    let comp = TemporalTensorCompressor::new(policy, len, now_ts);

    with_store(|store| {
        // Find a free slot
        for (i, slot) in store.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(comp);
                if !out_handle.is_null() {
                    unsafe { *out_handle = i as u32 };
                }
                return;
            }
        }
        // No free slot, push
        let idx = store.len();
        store.push(Some(comp));
        if !out_handle.is_null() {
            unsafe { *out_handle = idx as u32 };
        }
    });
}

/// Create a compressor with custom policy parameters.
#[no_mangle]
pub extern "C" fn ttc_create_with_policy(
    len: u32,
    now_ts: u32,
    hot_min_score: u32,
    warm_min_score: u32,
    warm_bits: u8,
    drift_pct_q8: u32,
    group_len: u32,
    out_handle: *mut u32,
) {
    let policy = TierPolicy {
        hot_min_score,
        warm_min_score,
        warm_bits,
        drift_pct_q8,
        group_len,
    };
    let comp = TemporalTensorCompressor::new(policy, len, now_ts);

    with_store(|store| {
        for (i, slot) in store.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(comp);
                if !out_handle.is_null() {
                    unsafe { *out_handle = i as u32 };
                }
                return;
            }
        }
        let idx = store.len();
        store.push(Some(comp));
        if !out_handle.is_null() {
            unsafe { *out_handle = idx as u32 };
        }
    });
}

/// Free a compressor.
#[no_mangle]
pub extern "C" fn ttc_free(handle: u32) {
    with_store(|store| {
        let idx = handle as usize;
        if idx < store.len() {
            store[idx] = None;
        }
    });
}

/// Record an access event.
#[no_mangle]
pub extern "C" fn ttc_touch(handle: u32, now_ts: u32) {
    with_compressor(handle, |comp| comp.touch(now_ts));
}

/// Set access stats directly.
#[no_mangle]
pub extern "C" fn ttc_set_access(handle: u32, access_count: u32, last_access_ts: u32) {
    with_compressor(handle, |comp| comp.set_access(access_count, last_access_ts));
}

/// Push a frame. If a segment boundary is crossed, the completed segment
/// is written to out_ptr/out_cap, and out_written is set to the byte count.
#[no_mangle]
pub extern "C" fn ttc_push_frame(
    handle: u32,
    now_ts: u32,
    in_ptr: *const f32,
    len: u32,
    out_ptr: *mut u8,
    out_cap: u32,
    out_written: *mut u32,
) {
    if out_written.is_null() {
        return;
    }
    unsafe { *out_written = 0 };
    if in_ptr.is_null() || out_ptr.is_null() {
        return;
    }

    let frame = unsafe { std::slice::from_raw_parts(in_ptr, len as usize) };
    let mut seg = Vec::new();

    with_compressor(handle, |comp| {
        comp.push_frame(frame, now_ts, &mut seg);
    });

    if seg.is_empty() || (seg.len() as u32) > out_cap {
        return;
    }

    unsafe {
        let out = std::slice::from_raw_parts_mut(out_ptr, out_cap as usize);
        out[..seg.len()].copy_from_slice(&seg);
        *out_written = seg.len() as u32;
    }
}

/// Flush the current segment.
#[no_mangle]
pub extern "C" fn ttc_flush(handle: u32, out_ptr: *mut u8, out_cap: u32, out_written: *mut u32) {
    if out_written.is_null() {
        return;
    }
    unsafe { *out_written = 0 };

    let mut seg = Vec::new();
    with_compressor(handle, |comp| {
        comp.flush(&mut seg);
    });

    if seg.is_empty() || out_ptr.is_null() || (seg.len() as u32) > out_cap {
        return;
    }

    unsafe {
        let out = std::slice::from_raw_parts_mut(out_ptr, out_cap as usize);
        out[..seg.len()].copy_from_slice(&seg);
        *out_written = seg.len() as u32;
    }
}

/// Decode a segment into f32 values.
#[no_mangle]
pub extern "C" fn ttc_decode_segment(
    seg_ptr: *const u8,
    seg_len: u32,
    out_ptr: *mut f32,
    out_cap_f32: u32,
    out_written_f32: *mut u32,
) {
    if out_written_f32.is_null() {
        return;
    }
    unsafe { *out_written_f32 = 0 };
    if seg_ptr.is_null() || out_ptr.is_null() {
        return;
    }

    let seg = unsafe { std::slice::from_raw_parts(seg_ptr, seg_len as usize) };
    let mut values = Vec::new();
    segment::decode(seg, &mut values);

    if values.is_empty() || (values.len() as u32) > out_cap_f32 {
        return;
    }

    unsafe {
        let out = std::slice::from_raw_parts_mut(out_ptr, out_cap_f32 as usize);
        out[..values.len()].copy_from_slice(&values);
        *out_written_f32 = values.len() as u32;
    }
}

/// Allocate a buffer in WASM linear memory.
#[no_mangle]
pub extern "C" fn ttc_alloc(size: u32, out_ptr: *mut u32) {
    if out_ptr.is_null() {
        return;
    }
    let mut v: Vec<u8> = Vec::with_capacity(size as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    unsafe {
        *out_ptr = p as u32;
    }
}

/// Free a buffer previously allocated with ttc_alloc.
#[no_mangle]
pub extern "C" fn ttc_dealloc(ptr: u32, cap: u32) {
    if ptr == 0 || cap == 0 {
        return;
    }
    unsafe {
        let _ = Vec::<u8>::from_raw_parts(ptr as *mut u8, 0, cap as usize);
    }
}

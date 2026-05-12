// On-ESP32-S3 temporal head — C ABI for the ESP-IDF firmware (ADR-095, #513).
//
// This crate is `staticlib` no_std + alloc. It is compiled to
// `xtensa-esp32s3-none-elf` and linked into the firmware via the ESP-IDF
// component glue in CMakeLists.txt. The host-side analog
// (`wifi-densepose-temporal`) tracks ADR-096; the two crates intentionally
// share the same `ruvllm_sparse_attention` kernel so behaviour is identical
// across host and node.
//
// Status (Phase 4 of #513): C ABI surface + ring buffer scaffold.
//   - `ruv_temporal_init`        ✓ scaffolded
//   - `ruv_temporal_push`        ✓ scaffolded (writes to ring buffer)
//   - `ruv_temporal_classify`    ✓ scaffolded (kernel forward stub)
//   - `ruv_temporal_destroy`     ✓ scaffolded
//
// Phase 5 wires real weights, panic_handler, and the global allocator to
// ESP-IDF's heap. Phase 6 wires the ABI calls from edge_processing.c into
// a dedicated FreeRTOS task per ADR-095 §3.3.

#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use core::ffi::c_void;

mod weights;
mod window;
use weights::{WeightBlobView, WeightLoadError};
use window::FrameRing;

// ---- ESP-IDF compatible error codes ---------------------------------------
//
// Matches the `esp_err_t` typedef in `esp_err.h`. We don't need the full
// set — these four cover the contract advertised in ruv_temporal.h.

const ESP_OK: i32 = 0;
const ESP_FAIL: i32 = -1;
const ESP_ERR_INVALID_ARG: i32 = 0x102;
const ESP_ERR_NO_MEM: i32 = 0x101;

// ---- Allocator ------------------------------------------------------------
//
// esp-alloc punches through to ESP-IDF's heap_caps_malloc. The ESP-IDF
// runtime calls `esp_alloc::HEAP.add_region(...)` from C startup before
// the first Rust allocation; without that wiring we'd hit OOM on the
// first Vec push. That wiring lands in Phase 5 along with the rest of
// the firmware-side glue.
#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

// ---- Panic handler --------------------------------------------------------
//
// Production firmware would route to ESP-IDF's `esp_system_abort` so the
// crash shows up in core dumps. For Phase 4 scaffolding we simply halt —
// keeps the staticlib self-contained without dragging in `esp-idf-sys`.

#[panic_handler]
fn on_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        // wait-for-interrupt would be nicer; this is fine until Phase 5
        // hooks into esp_system_abort.
    }
}

// ---- Context object (opaque to C callers) ---------------------------------

pub struct RuvTemporalCtx {
    input_dim: u32,
    window_len: u32,
    n_classes: u32,
    ring: FrameRing,
}

// ---- Public C ABI ---------------------------------------------------------

/// Initialise a temporal-head context. Allocates and returns an opaque
/// pointer through `out_ctx`. Returns ESP_OK on success, an esp_err_t on
/// failure. Caller must release with `ruv_temporal_destroy`.
#[no_mangle]
pub extern "C" fn ruv_temporal_init(
    weights: *const u8,
    weights_len: usize,
    input_dim: u32,
    window_len: u32,
    n_classes: u32,
    out_ctx: *mut *mut RuvTemporalCtx,
) -> i32 {
    if out_ctx.is_null() || input_dim == 0 || window_len == 0 || n_classes == 0 {
        return ESP_ERR_INVALID_ARG;
    }

    // Optional weights blob: when caller passes a non-NULL pointer,
    // parse and validate it. Caller can pass NULL during the Phase 4/5
    // bring-up window when the kernel forward isn't actually consuming
    // weights yet — we just want the parse path itself proven on the
    // device. Once Phase 5 unblocks and the kernel is wired, Phase 6
    // makes a non-NULL weights argument required.
    if !weights.is_null() && weights_len > 0 {
        // SAFETY: caller asserts the buffer covers `weights_len` bytes
        // and outlives this call. Borrowed-slice parse — no copy.
        let buf = unsafe { core::slice::from_raw_parts(weights, weights_len) };
        match WeightBlobView::parse(buf) {
            Ok(view) => {
                // Sanity-check that the blob's declared shape matches
                // the runtime arguments. A blob with input_dim=32 in
                // a context configured for input_dim=16 is a deploy bug
                // we want to catch at init() not at first classify().
                if view.header.input_dim as u32 != input_dim
                    || view.header.n_classes as u32 != n_classes
                {
                    return ESP_ERR_INVALID_ARG;
                }
                // Phase 5+: stash view into the context for the kernel
                // to consume. For now the parse itself is the proof
                // that the format crossed the host/firmware boundary.
            }
            Err(e) => return weights::weight_load_err_to_esp(&e),
        }
    }

    let ring = match FrameRing::new(window_len as usize, input_dim as usize) {
        Some(r) => r,
        None => return ESP_ERR_NO_MEM,
    };

    let ctx = Box::new(RuvTemporalCtx {
        input_dim,
        window_len,
        n_classes,
        ring,
    });
    unsafe { *out_ctx = Box::into_raw(ctx) };
    ESP_OK
}

/// Push one feature frame into the rolling window. Hot path — must stay
/// cheap (no allocation, no kernel work).
#[no_mangle]
pub extern "C" fn ruv_temporal_push(ctx: *mut RuvTemporalCtx, frame: *const f32) -> i32 {
    if ctx.is_null() || frame.is_null() {
        return ESP_ERR_INVALID_ARG;
    }
    let ctx = unsafe { &mut *ctx };
    let slice = unsafe { core::slice::from_raw_parts(frame, ctx.input_dim as usize) };
    ctx.ring.push(slice);
    ESP_OK
}

/// Run the temporal-head forward and write `n_classes` logits into the
/// caller-owned `logits` buffer. Returns ESP_OK on success.
///
/// Phase 4 stub: writes a zero-vector. Phase 5 wires the real
/// `SubquadraticSparseAttention::forward_gqa` over the ring buffer
/// contents. The signature is what edge_processing.c will call — that
/// part of the contract is stable now.
#[no_mangle]
pub extern "C" fn ruv_temporal_classify(
    ctx: *mut RuvTemporalCtx,
    logits: *mut f32,
    n_classes: u32,
) -> i32 {
    if ctx.is_null() || logits.is_null() {
        return ESP_ERR_INVALID_ARG;
    }
    let ctx = unsafe { &*ctx };
    if n_classes != ctx.n_classes {
        return ESP_ERR_INVALID_ARG;
    }
    let out = unsafe { core::slice::from_raw_parts_mut(logits, n_classes as usize) };
    for slot in out.iter_mut() {
        *slot = 0.0;
    }
    let _ = ctx.window_len; // future: feed ring -> attention -> classifier head
    ESP_OK
}

/// Release a context allocated by `ruv_temporal_init`.
#[no_mangle]
pub extern "C" fn ruv_temporal_destroy(ctx: *mut RuvTemporalCtx) {
    if ctx.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(ctx));
    }
}

// ---- Static guard ---------------------------------------------------------
//
// Force a *use* of the upstream crate so the link line proves the crate is
// reachable from the staticlib. Without this the compiler may strip the
// dependency entirely in Phase 4 since classify() doesn't yet call into it.
#[doc(hidden)]
#[no_mangle]
pub extern "C" fn ruv_temporal_kernel_self_test() -> i32 {
    use ruvllm_sparse_attention::{SparseAttentionConfig, SubquadraticSparseAttention, Tensor3};
    let cfg = SparseAttentionConfig {
        window: 4,
        block_size: 2,
        global_tokens: alloc::vec![0],
        causal: true,
        use_log_stride: true,
        use_landmarks: true,
        sort_candidates: false,
    };
    if SubquadraticSparseAttention::new(cfg).is_err() {
        return ESP_FAIL;
    }
    let _ = Tensor3::zeros(0, 1, 1);
    ESP_OK
}

// Prevent dead-code drop of the C ABI when the linker is aggressive.
#[used]
static _ABI_KEEPALIVE: [extern "C" fn(); 5] = [
    keepalive_init,
    keepalive_push,
    keepalive_classify,
    keepalive_destroy,
    keepalive_self_test,
];

extern "C" fn keepalive_init() {
    let _ = ruv_temporal_init;
}
extern "C" fn keepalive_push() {
    let _ = ruv_temporal_push;
}
extern "C" fn keepalive_classify() {
    let _ = ruv_temporal_classify;
}
extern "C" fn keepalive_destroy() {
    let _ = ruv_temporal_destroy;
}
extern "C" fn keepalive_self_test() {
    let _ = ruv_temporal_kernel_self_test;
}

// Avoid "unused" warnings on the c_void import while the actual handle
// type is what callers receive.
const _: Option<*const c_void> = None;

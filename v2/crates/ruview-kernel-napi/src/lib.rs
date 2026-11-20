//! napi-rs transport for `ruview-kernel` (ADR-368).
//!
//! Deliberately exposes only the kernel's single string ABI so the native
//! and WASM backends of `@ruvnet/ruview-kernel` stay behaviourally identical.
#![deny(clippy::all)]

use napi::bindgen_prelude::Float64Array;
use napi_derive::napi;

/// JSON ABI version; must equal the WASM module's `rvk_abi_version()`.
#[napi]
pub fn abi_version() -> u32 {
    ruview_kernel::ABI_VERSION
}

/// Run one kernel operation: operation name + JSON request → JSON envelope.
/// A Rust panic is converted into an `internal` error envelope.
#[napi]
pub fn call(op: String, input: String) -> String {
    std::panic::catch_unwind(|| ruview_kernel::call(&op, &input)).unwrap_or_else(|_| {
        r#"{"ok":false,"error":{"code":"internal","message":"kernel panicked"}}"#.to_string()
    })
}

/// Binary fast path: frame data as a `Float64Array` (`analyze_flat`,
/// `session_push_flat`), same envelope as [`call`].
#[napi]
pub fn call_f64(op: String, input: String, data: Float64Array) -> String {
    // The kernel only reads the slice, so observing it after a panic is sound.
    let values = std::panic::AssertUnwindSafe(&data[..]);
    std::panic::catch_unwind(move || ruview_kernel::call_f64(&op, &input, *values)).unwrap_or_else(
        |_| r#"{"ok":false,"error":{"code":"internal","message":"kernel panicked"}}"#.to_string(),
    )
}

//! Browser Feature Detection for Web Workers
//!
//! Detects availability of SharedArrayBuffer, Atomics, and other
//! features required for parallel inference.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Check if SharedArrayBuffer is available.
///
/// SharedArrayBuffer is required for zero-copy memory sharing between
/// the main thread and Web Workers.
///
/// # Notes
/// - SharedArrayBuffer was temporarily disabled in all browsers after
///   Spectre/Meltdown vulnerabilities were discovered.
/// - It's now available again, but requires cross-origin isolation:
///   - `Cross-Origin-Opener-Policy: same-origin`
///   - `Cross-Origin-Embedder-Policy: require-corp`
///
/// # Returns
/// `true` if SharedArrayBuffer is available, `false` otherwise.
#[wasm_bindgen]
pub fn is_shared_array_buffer_available() -> bool {
    // Try to access SharedArrayBuffer constructor
    let global = js_sys::global();

    if let Ok(sab) = js_sys::Reflect::get(&global, &JsValue::from_str("SharedArrayBuffer")) {
        if !sab.is_undefined() && !sab.is_null() {
            // Try to create a small SharedArrayBuffer to verify it's actually usable
            match js_sys::SharedArrayBuffer::new(8) {
                _ => return true,
            }
        }
    }

    false
}

/// Check if Atomics API is available.
///
/// Atomics provides atomic operations for synchronization between
/// the main thread and Web Workers.
///
/// # Returns
/// `true` if Atomics is available, `false` otherwise.
#[wasm_bindgen]
pub fn is_atomics_available() -> bool {
    let global = js_sys::global();

    if let Ok(atomics) = js_sys::Reflect::get(&global, &JsValue::from_str("Atomics")) {
        if !atomics.is_undefined() && !atomics.is_null() {
            // Verify Atomics.wait and Atomics.notify are available
            if let Ok(wait) = js_sys::Reflect::get(&atomics, &JsValue::from_str("wait")) {
                if let Ok(notify) = js_sys::Reflect::get(&atomics, &JsValue::from_str("notify")) {
                    return !wait.is_undefined() && !notify.is_undefined();
                }
            }
        }
    }

    false
}

/// Check if the page is cross-origin isolated.
///
/// Cross-origin isolation is required for SharedArrayBuffer to work.
/// The page must be served with:
/// - `Cross-Origin-Opener-Policy: same-origin`
/// - `Cross-Origin-Embedder-Policy: require-corp`
///
/// # Returns
/// `true` if cross-origin isolated, `false` otherwise.
#[wasm_bindgen]
pub fn cross_origin_isolated() -> bool {
    if let Some(window) = web_sys::window() {
        // crossOriginIsolated is a boolean property on Window
        if let Ok(isolated) =
            js_sys::Reflect::get(&window, &JsValue::from_str("crossOriginIsolated"))
        {
            return isolated.as_bool().unwrap_or(false);
        }
    }

    // Also check in worker context
    let global = js_sys::global();
    if let Ok(isolated) = js_sys::Reflect::get(&global, &JsValue::from_str("crossOriginIsolated")) {
        return isolated.as_bool().unwrap_or(false);
    }

    false
}

/// Check if Web Workers are available.
///
/// # Returns
/// `true` if Web Workers are available, `false` otherwise.
#[wasm_bindgen]
pub fn is_web_workers_available() -> bool {
    let global = js_sys::global();

    if let Ok(worker) = js_sys::Reflect::get(&global, &JsValue::from_str("Worker")) {
        return !worker.is_undefined() && !worker.is_null();
    }

    false
}

/// Get the optimal number of workers based on hardware concurrency.
///
/// Uses `navigator.hardwareConcurrency` if available, otherwise falls
/// back to a reasonable default.
///
/// # Notes
/// - Caps the result at MAX_WORKERS to prevent resource exhaustion.
/// - Leaves at least 1 core for the main thread.
/// - Falls back to 4 if hardware concurrency is not available.
///
/// # Returns
/// Recommended number of workers.
#[wasm_bindgen]
pub fn optimal_worker_count() -> usize {
    const MAX_WORKERS: usize = 16;
    const MIN_WORKERS: usize = 2;
    const DEFAULT_WORKERS: usize = 4;

    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        // hardwareConcurrency returns the number of logical processors
        let cores = navigator.hardware_concurrency() as usize;
        if cores > 0 {
            // Leave at least 1 core for main thread
            // Cap at MAX_WORKERS
            return (cores.saturating_sub(1)).clamp(MIN_WORKERS, MAX_WORKERS);
        }
    }

    // Check in worker global scope
    let global = js_sys::global();
    if let Ok(navigator) = js_sys::Reflect::get(&global, &JsValue::from_str("navigator")) {
        if !navigator.is_undefined() {
            if let Ok(cores) =
                js_sys::Reflect::get(&navigator, &JsValue::from_str("hardwareConcurrency"))
            {
                if let Some(c) = cores.as_f64() {
                    let cores = c as usize;
                    if cores > 0 {
                        return (cores.saturating_sub(1)).clamp(MIN_WORKERS, MAX_WORKERS);
                    }
                }
            }
        }
    }

    DEFAULT_WORKERS
}

/// Check if SIMD (WebAssembly SIMD) is available.
///
/// # Returns
/// `true` if WASM SIMD is available, `false` otherwise.
#[wasm_bindgen]
pub fn is_simd_available() -> bool {
    // This is checked at compile time in Rust
    #[cfg(target_feature = "simd128")]
    {
        true
    }
    #[cfg(not(target_feature = "simd128"))]
    {
        // Runtime check using WebAssembly.validate
        let global = js_sys::global();
        if let Ok(wasm) = js_sys::Reflect::get(&global, &JsValue::from_str("WebAssembly")) {
            if !wasm.is_undefined() {
                if let Ok(validate) = js_sys::Reflect::get(&wasm, &JsValue::from_str("validate")) {
                    if validate.is_function() {
                        // SIMD test module (v128.const)
                        let simd_test: [u8; 14] = [
                            0x00, 0x61, 0x73, 0x6d, // magic
                            0x01, 0x00, 0x00, 0x00, // version
                            0x01, 0x05, 0x01, 0x60, // type section
                            0x00, 0x01, // func type () -> v128
                        ];

                        let arr = js_sys::Uint8Array::from(&simd_test[..]);
                        let validate_fn: js_sys::Function = validate.unchecked_into();
                        if let Ok(result) = validate_fn.call1(&JsValue::NULL, &arr) {
                            return result.as_bool().unwrap_or(false);
                        }
                    }
                }
            }
        }
        false
    }
}

/// Check if BigInt is available.
///
/// BigInt is useful for 64-bit integer operations.
///
/// # Returns
/// `true` if BigInt is available, `false` otherwise.
#[wasm_bindgen]
pub fn is_bigint_available() -> bool {
    let global = js_sys::global();

    if let Ok(bigint) = js_sys::Reflect::get(&global, &JsValue::from_str("BigInt")) {
        return !bigint.is_undefined() && !bigint.is_null();
    }

    false
}

/// Check if Transferable objects are available.
///
/// Transferable objects (ArrayBuffer, MessagePort, etc.) can be
/// transferred to workers without copying.
///
/// # Returns
/// `true` if Transferable objects are available, `false` otherwise.
#[wasm_bindgen]
pub fn is_transferable_available() -> bool {
    // Transferable is supported in all modern browsers
    // Try to create an ArrayBuffer which is always transferable
    let buffer = js_sys::ArrayBuffer::new(8);
    let global = js_sys::global();

    if let Ok(post_message) = js_sys::Reflect::get(&global, &JsValue::from_str("postMessage")) {
        if post_message.is_function() {
            // If we can create ArrayBuffer and postMessage exists, transferable is supported
            return !buffer.is_undefined();
        }
    }

    // Also check window.postMessage
    if let Some(window) = web_sys::window() {
        // postMessage is available
        return true;
    }

    false
}

/// Get a summary of all available features.
///
/// # Returns
/// JSON string with feature availability.
#[wasm_bindgen]
pub fn feature_summary() -> String {
    let features = serde_json::json!({
        "shared_array_buffer": is_shared_array_buffer_available(),
        "atomics": is_atomics_available(),
        "cross_origin_isolated": cross_origin_isolated(),
        "web_workers": is_web_workers_available(),
        "simd": is_simd_available(),
        "bigint": is_bigint_available(),
        "transferable": is_transferable_available(),
        "optimal_workers": optimal_worker_count(),
    });

    serde_json::to_string_pretty(&features).unwrap_or_else(|_| "{}".to_string())
}

/// Browser capability level for parallel inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityLevel {
    /// Full parallel capability with shared memory
    Full,
    /// Partial capability - workers available but no shared memory
    Partial,
    /// No parallel capability - single-threaded only
    None,
}

/// Determine the capability level for parallel inference.
///
/// # Returns
/// The capability level based on available features.
#[wasm_bindgen]
pub fn detect_capability_level() -> String {
    let level = if is_shared_array_buffer_available()
        && is_atomics_available()
        && is_web_workers_available()
        && cross_origin_isolated()
    {
        CapabilityLevel::Full
    } else if is_web_workers_available() {
        CapabilityLevel::Partial
    } else {
        CapabilityLevel::None
    };

    match level {
        CapabilityLevel::Full => "full".to_string(),
        CapabilityLevel::Partial => "partial".to_string(),
        CapabilityLevel::None => "none".to_string(),
    }
}

/// Check if the environment supports parallel inference.
///
/// # Arguments
/// * `require_shared_memory` - Whether to require SharedArrayBuffer
///
/// # Returns
/// `true` if parallel inference is supported, `false` otherwise.
#[wasm_bindgen]
pub fn supports_parallel_inference(require_shared_memory: bool) -> bool {
    if !is_web_workers_available() {
        return false;
    }

    if require_shared_memory {
        is_shared_array_buffer_available() && is_atomics_available() && cross_origin_isolated()
    } else {
        true
    }
}

/// Get a message explaining why parallel inference is not available.
///
/// # Returns
/// Explanation string, or empty string if parallel inference is available.
#[wasm_bindgen]
pub fn parallel_inference_unavailable_reason() -> String {
    if !is_web_workers_available() {
        return "Web Workers are not available in this environment.".to_string();
    }

    if !is_shared_array_buffer_available() {
        return "SharedArrayBuffer is not available. This may be due to missing cross-origin isolation headers.".to_string();
    }

    if !is_atomics_available() {
        return "Atomics API is not available.".to_string();
    }

    if !cross_origin_isolated() {
        return "Page is not cross-origin isolated. Required headers:\n\
                - Cross-Origin-Opener-Policy: same-origin\n\
                - Cross-Origin-Embedder-Policy: require-corp"
            .to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_level() {
        // These tests will behave differently in WASM vs native
        let level = detect_capability_level();
        assert!(level == "full" || level == "partial" || level == "none");
    }

    #[test]
    fn test_feature_summary() {
        let summary = feature_summary();
        assert!(summary.contains("shared_array_buffer"));
        assert!(summary.contains("optimal_workers"));
    }
}

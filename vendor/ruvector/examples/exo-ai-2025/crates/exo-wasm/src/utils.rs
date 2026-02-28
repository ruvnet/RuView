//! Utility functions for WASM runtime
//!
//! This module provides utility functions for panic handling, logging,
//! and browser environment detection.

use wasm_bindgen::prelude::*;
use web_sys::console;

/// Set up panic hook for better error messages in browser console
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Log a message to the browser console
pub fn log(message: &str) {
    console::log_1(&JsValue::from_str(message));
}

/// Log a warning to the browser console
pub fn warn(message: &str) {
    console::warn_1(&JsValue::from_str(message));
}

/// Log an error to the browser console
pub fn error(message: &str) {
    console::error_1(&JsValue::from_str(message));
}

/// Log debug information (includes timing)
pub fn debug(message: &str) {
    console::debug_1(&JsValue::from_str(message));
}

/// Measure execution time of a function
pub fn measure_time<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = js_sys::Date::now();
    let result = f();
    let elapsed = js_sys::Date::now() - start;
    log(&format!("{} took {:.2}ms", name, elapsed));
    result
}

/// Check if running in a Web Worker context
#[wasm_bindgen]
pub fn is_web_worker() -> bool {
    js_sys::eval("typeof WorkerGlobalScope !== 'undefined'")
        .map(|v| v.is_truthy())
        .unwrap_or(false)
}

/// Check if running in a browser with WebAssembly support
#[wasm_bindgen]
pub fn is_wasm_supported() -> bool {
    js_sys::eval("typeof WebAssembly !== 'undefined'")
        .map(|v| v.is_truthy())
        .unwrap_or(false)
}

/// Get browser performance metrics
#[wasm_bindgen]
pub fn get_performance_metrics() -> Result<JsValue, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;
    let performance = window
        .performance()
        .ok_or_else(|| JsValue::from_str("No performance object"))?;

    let timing = performance.timing();

    let metrics = serde_json::json!({
        "navigation_start": timing.navigation_start(),
        "dom_complete": timing.dom_complete(),
        "load_event_end": timing.load_event_end(),
    });

    serde_wasm_bindgen::to_value(&metrics)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize metrics: {}", e)))
}

/// Get available memory (if supported by browser)
#[wasm_bindgen]
pub fn get_memory_info() -> Result<JsValue, JsValue> {
    // Try to access performance.memory (Chrome only)
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;
    let performance = window
        .performance()
        .ok_or_else(|| JsValue::from_str("No performance object"))?;

    // This is non-standard and may not be available
    let result = js_sys::Reflect::get(&performance, &JsValue::from_str("memory"));

    if let Ok(memory) = result {
        if !memory.is_undefined() {
            return Ok(memory);
        }
    }

    // Fallback: return empty object
    Ok(js_sys::Object::new().into())
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: f64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Generate a random UUID v4
#[wasm_bindgen]
pub fn generate_uuid() -> String {
    // Use crypto.randomUUID if available, otherwise fallback
    let result = js_sys::eval(
        "typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : null"
    );

    if let Ok(uuid) = result {
        if let Some(uuid_str) = uuid.as_string() {
            return uuid_str;
        }
    }

    // Fallback: simple UUID generation
    use getrandom::getrandom;
    let mut bytes = [0u8; 16];
    if getrandom(&mut bytes).is_ok() {
        // Set version (4) and variant bits
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;

        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5],
            bytes[6], bytes[7],
            bytes[8], bytes[9],
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        )
    } else {
        // Ultimate fallback: timestamp-based ID
        format!("{}-{}", js_sys::Date::now(), js_sys::Math::random())
    }
}

/// Check if localStorage is available
#[wasm_bindgen]
pub fn is_local_storage_available() -> bool {
    js_sys::eval("typeof localStorage !== 'undefined'")
        .map(|v| v.is_truthy())
        .unwrap_or(false)
}

/// Check if IndexedDB is available
#[wasm_bindgen]
pub fn is_indexed_db_available() -> bool {
    js_sys::eval("typeof indexedDB !== 'undefined'")
        .map(|v| v.is_truthy())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(100.0), "100.00 B");
        assert_eq!(format_bytes(1024.0), "1.00 KB");
        assert_eq!(format_bytes(1024.0 * 1024.0), "1.00 MB");
    }
}

//! Utility functions for WASM environment
//!
//! Provides helper functions for panic handling, logging, and
//! JavaScript interop utilities.

use wasm_bindgen::prelude::*;

/// Set panic hook for better error messages in the browser console.
///
/// This function should be called once at initialization to enable
/// better panic messages in the browser's developer console.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm_wasm::utils::set_panic_hook;
///
/// // Call at app startup
/// set_panic_hook();
/// ```
pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Log a message to the browser console.
///
/// # Arguments
///
/// * `message` - The message to log
#[wasm_bindgen]
pub fn log(message: &str) {
    web_sys::console::log_1(&message.into());
}

/// Log a warning to the browser console.
///
/// # Arguments
///
/// * `message` - The warning message
#[wasm_bindgen]
pub fn warn(message: &str) {
    web_sys::console::warn_1(&message.into());
}

/// Log an error to the browser console.
///
/// # Arguments
///
/// * `message` - The error message
#[wasm_bindgen]
pub fn error(message: &str) {
    web_sys::console::error_1(&message.into());
}

/// Get current timestamp in milliseconds using Performance API.
///
/// Returns high-resolution timestamp for performance measurements.
#[wasm_bindgen]
pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// Simple timer for measuring elapsed time in WASM.
#[wasm_bindgen]
pub struct Timer {
    start: f64,
    label: String,
}

#[wasm_bindgen]
impl Timer {
    /// Create a new timer with the given label.
    ///
    /// # Arguments
    ///
    /// * `label` - A descriptive label for the timer
    #[wasm_bindgen(constructor)]
    pub fn new(label: &str) -> Timer {
        Timer {
            start: now_ms(),
            label: label.to_string(),
        }
    }

    /// Get elapsed time in milliseconds.
    #[wasm_bindgen]
    pub fn elapsed_ms(&self) -> f64 {
        now_ms() - self.start
    }

    /// Log elapsed time to console and return the duration.
    #[wasm_bindgen]
    pub fn stop(&self) -> f64 {
        let elapsed = self.elapsed_ms();
        log(&format!("{}: {:.2}ms", self.label, elapsed));
        elapsed
    }

    /// Reset the timer.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.start = now_ms();
    }
}

/// Convert a Rust Result to a JavaScript-friendly format.
///
/// On success, returns the value. On error, throws a JavaScript exception.
pub fn result_to_js<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, JsValue> {
    result.map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // set_panic_hook requires console_error_panic_hook which only works on wasm32
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_set_panic_hook() {
        // Should not panic
        set_panic_hook();
    }

    // Non-wasm32 version just verifies the function exists
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_set_panic_hook_noop() {
        // On non-wasm32, this is a no-op
        set_panic_hook();
    }
}

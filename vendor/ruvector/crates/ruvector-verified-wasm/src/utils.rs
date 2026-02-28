//! WASM utility helpers.

/// Set panic hook for better panic messages in the browser.
pub fn set_panic_hook() {
    // No-op if console_error_panic_hook is not available.
    // In production, add the crate and feature for better diagnostics.
}

/// Log a message to the browser console.
pub fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

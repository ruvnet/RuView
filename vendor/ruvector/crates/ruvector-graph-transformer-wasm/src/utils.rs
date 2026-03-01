//! WASM utility helpers.

/// Set panic hook for better panic messages in the browser.
///
/// No-op; add `console_error_panic_hook` as an optional dependency for
/// improved browser diagnostics.
pub fn set_panic_hook() {
    // Intentional no-op. In production, wire up console_error_panic_hook.
}

//! WiFi-DensePose WebAssembly bindings
//!
//! This crate provides WebAssembly bindings for browser-based applications using
//! WiFi-DensePose technology. It includes:
//!
//! - **mat**: WiFi-Mat disaster response dashboard module for browser integration
//!
//! # Features
//!
//! - `mat` - Enable WiFi-Mat disaster detection WASM bindings
//! - `console_error_panic_hook` - Better panic messages in browser console
//!
//! # Building for WASM
//!
//! ```bash
//! # Build with wasm-pack
//! wasm-pack build --target web --features mat
//!
//! # Or with cargo
//! cargo build --target wasm32-unknown-unknown --features mat
//! ```
//!
//! # Example Usage (JavaScript)
//!
//! ```javascript
//! import init, { MatDashboard, initLogging } from './wifi_densepose_wasm.js';
//!
//! async function main() {
//!     await init();
//!     initLogging('info');
//!
//!     const dashboard = new MatDashboard();
//!
//!     // Create a disaster event
//!     const eventId = dashboard.createEvent('earthquake', 37.7749, -122.4194, 'Bay Area Earthquake');
//!
//!     // Add scan zones
//!     dashboard.addRectangleZone('Building A', 50, 50, 200, 150);
//!     dashboard.addCircleZone('Search Area B', 400, 200, 80);
//!
//!     // Subscribe to events
//!     dashboard.onSurvivorDetected((survivor) => {
//!         console.log('Survivor detected:', survivor);
//!         updateUI(survivor);
//!     });
//!
//!     dashboard.onAlertGenerated((alert) => {
//!         showNotification(alert);
//!     });
//!
//!     // Render to canvas
//!     const canvas = document.getElementById('map');
//!     const ctx = canvas.getContext('2d');
//!
//!     function render() {
//!         ctx.clearRect(0, 0, canvas.width, canvas.height);
//!         dashboard.renderZones(ctx);
//!         dashboard.renderSurvivors(ctx);
//!         requestAnimationFrame(render);
//!     }
//!     render();
//! }
//!
//! main();
//! ```

use wasm_bindgen::prelude::*;

// WiFi-Mat module for disaster response dashboard
pub mod mat;
pub use mat::*;

/// Initialize the WASM module.
/// Call this once at startup before using any other functions.
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages in browser console
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Initialize logging with specified level.
///
/// @param {string} level - Log level: "trace", "debug", "info", "warn", "error"
#[wasm_bindgen(js_name = initLogging)]
pub fn init_logging(level: &str) {
    let log_level = match level.to_lowercase().as_str() {
        "trace" => log::Level::Trace,
        "debug" => log::Level::Debug,
        "info" => log::Level::Info,
        "warn" => log::Level::Warn,
        "error" => log::Level::Error,
        _ => log::Level::Info,
    };

    let _ = wasm_logger::init(wasm_logger::Config::new(log_level));
    log::info!("WiFi-DensePose WASM initialized with log level: {}", level);
}

/// Get the library version.
///
/// @returns {string} Version string
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if the MAT feature is enabled.
///
/// @returns {boolean} True if MAT module is available
#[wasm_bindgen(js_name = isMatEnabled)]
pub fn is_mat_enabled() -> bool {
    true
}

/// Get current timestamp in milliseconds (for performance measurements).
///
/// @returns {number} Timestamp in milliseconds
#[wasm_bindgen(js_name = getTimestamp)]
pub fn get_timestamp() -> f64 {
    let window = web_sys::window().expect("no global window");
    let performance = window.performance().expect("no performance object");
    performance.now()
}

// Re-export all public types from mat module for easy access
pub mod types {
    pub use super::mat::{
        JsAlert, JsAlertPriority, JsDashboardStats, JsDisasterType, JsScanZone, JsSurvivor,
        JsTriageStatus, JsZoneStatus,
    };
}

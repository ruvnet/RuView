//! Host WiFi RSSI capture and processing tasks.

#[cfg(not(target_os = "macos"))]
pub mod fallback;
pub mod task;

#[cfg(not(target_os = "macos"))]
pub use fallback::windows_wifi_fallback_tick;
pub use task::wifi_task;


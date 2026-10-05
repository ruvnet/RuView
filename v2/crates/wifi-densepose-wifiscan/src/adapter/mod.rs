//! Adapter implementations for the [`WlanScanPort`] port.
//!
//! Each adapter targets a specific platform scanning mechanism:
//! - [`NetshBssidScanner`]: Tier 1 -- parses `netsh wlan show networks mode=bssid` (Windows).
//! - [`WlanApiScanner`]: Tier 2 -- native `wlanapi.dll` BSS-list FFI with a
//!   `netsh` fallback, metrics, and a measured-rate benchmark (Windows).
//! - [`MacosCoreWlanScanner`]: CoreWLAN via Swift helper binary (macOS, ADR-025).
//! - [`LinuxIwScanner`]: parses `iw dev <iface> scan` output (Linux).

pub(crate) mod netsh_scanner;
/// Native `wlanapi.dll` BSS-list FFI (real on Windows, typed `Unsupported`
/// elsewhere). Backs the Tier 2 native scan path.
pub(crate) mod wlanapi_native;
pub mod wlanapi_scanner;

#[cfg(target_os = "macos")]
pub mod macos_scanner;

#[cfg(target_os = "linux")]
pub mod linux_scanner;

pub use netsh_scanner::parse_netsh_output;
pub use netsh_scanner::NetshBssidScanner;
pub use wlanapi_native::request_scan;
pub use wlanapi_scanner::WlanApiScanner;

#[cfg(target_os = "macos")]
pub use macos_scanner::parse_macos_scan_output;
#[cfg(target_os = "macos")]
pub use macos_scanner::MacosCoreWlanScanner;

#[cfg(target_os = "linux")]
pub use linux_scanner::parse_iw_scan_output;
#[cfg(target_os = "linux")]
pub use linux_scanner::LinuxIwScanner;

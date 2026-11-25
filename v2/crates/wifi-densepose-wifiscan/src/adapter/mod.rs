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

/// All platform adapters compile on every target; the actual system call
/// fails at runtime with a typed [`WifiScanError`](crate::error::WifiScanError)
/// where the backend does not exist. This keeps `wifi_task`'s platform
/// branches type-checkable in CI on any OS.
pub mod macos_scanner;

pub mod linux_scanner;

pub use netsh_scanner::parse_netsh_output;
pub use netsh_scanner::NetshBssidScanner;
pub use wlanapi_scanner::WlanApiScanner;

pub use macos_scanner::parse_macos_scan_output;
pub use macos_scanner::MacosCoreWlanScanner;

pub use linux_scanner::detect_interface;
pub use linux_scanner::parse_iw_dev_interfaces;
pub use linux_scanner::parse_iw_scan_output;
pub use linux_scanner::LinuxIwScanner;

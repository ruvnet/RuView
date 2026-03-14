//! Adapter implementations for the [`WlanScanPort`] port.
//!
//! Each adapter targets a specific platform scanning mechanism:
//! - [`NetshBssidScanner`]: Tier 1 -- parses `netsh wlan show networks mode=bssid` (Windows).
//! - [`WlanApiScanner`]: Tier 2 -- async wrapper with metrics and future native FFI path (Windows).
//! - [`MacosCoreWlanScanner`]: CoreWLAN via Swift helper binary (macOS, ADR-025).
//! - [`LinuxIwScanner`]: parses `iw dev <iface> scan` output (Linux).

pub mod macos_scanner;
pub(crate) mod netsh_scanner;
pub mod wlanapi_scanner;

#[cfg(target_os = "linux")]
pub mod linux_scanner;

pub use macos_scanner::parse_macos_scan_output;
pub use macos_scanner::MacosCoreWlanScanner;
pub use netsh_scanner::parse_netsh_output;
pub use netsh_scanner::NetshBssidScanner;
pub use wlanapi_scanner::WlanApiScanner;

#[cfg(target_os = "linux")]
pub use linux_scanner::parse_iw_scan_output;
#[cfg(target_os = "linux")]
pub use linux_scanner::LinuxIwScanner;

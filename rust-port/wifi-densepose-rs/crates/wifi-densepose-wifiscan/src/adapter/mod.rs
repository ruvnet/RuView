//! Adapter implementations for the [`WlanScanPort`] port.
//!
//! Each adapter targets a specific platform scanning mechanism:
//! - [`NetshBssidScanner`]: Tier 1 -- parses `netsh wlan show networks mode=bssid`.
//! - [`WlanApiScanner`]: Tier 2 -- async wrapper with metrics and future native FFI path.

pub(crate) mod netsh_scanner;
pub mod wlanapi_scanner;

pub use netsh_scanner::NetshBssidScanner;
pub use netsh_scanner::parse_netsh_output;
pub use wlanapi_scanner::WlanApiScanner;

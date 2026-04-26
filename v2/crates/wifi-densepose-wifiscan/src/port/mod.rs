//! Port definitions for the BSSID Acquisition bounded context.
//!
//! Hexagonal-architecture ports that abstract the WiFi scanning backend,
//! enabling Tier 1 (netsh), Tier 2 (wlanapi FFI), and test-double adapters
//! to be swapped transparently.

mod scan_port;

pub use scan_port::WlanScanPort;

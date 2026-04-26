//! # wifi-densepose-wifiscan
//!
//! Domain layer for multi-BSSID WiFi scanning and enhanced sensing (ADR-022).
//!
//! This crate implements the **BSSID Acquisition** bounded context, providing:
//!
//! - **Domain types**: [`BssidId`], [`BssidObservation`], [`BandType`], [`RadioType`]
//! - **Port**: [`WlanScanPort`] -- trait abstracting the platform scan backend
//! - **Adapters**:
//!   - [`NetshBssidScanner`] -- Windows, parses `netsh wlan show networks mode=bssid`
//!   - `MacosCoreWlanScanner` -- macOS, invokes CoreWLAN Swift helper (ADR-025)
//!   - `LinuxIwScanner` -- Linux, parses `iw dev <iface> scan` output

pub mod adapter;
pub mod domain;
pub mod error;
pub mod pipeline;
pub mod port;

// Re-export key types at the crate root for convenience.
pub use adapter::NetshBssidScanner;
pub use adapter::parse_netsh_output;
pub use adapter::WlanApiScanner;

#[cfg(target_os = "macos")]
pub use adapter::MacosCoreWlanScanner;
#[cfg(target_os = "macos")]
pub use adapter::parse_macos_scan_output;

#[cfg(target_os = "linux")]
pub use adapter::LinuxIwScanner;
#[cfg(target_os = "linux")]
pub use adapter::parse_iw_scan_output;
pub use domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
pub use domain::frame::MultiApFrame;
pub use domain::registry::{BssidEntry, BssidMeta, BssidRegistry, RunningStats};
pub use domain::result::EnhancedSensingResult;
pub use error::WifiScanError;
pub use port::WlanScanPort;

#[cfg(feature = "pipeline")]
pub use pipeline::WindowsWifiPipeline;

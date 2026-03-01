//! # wifi-densepose-wifiscan
//!
//! Domain layer for multi-BSSID WiFi scanning and enhanced sensing (ADR-022).
//!
//! This crate implements the **BSSID Acquisition** bounded context, providing:
//!
//! - **Domain types**: [`BssidId`], [`BssidObservation`], [`BandType`], [`RadioType`]
//! - **Port**: [`WlanScanPort`] -- trait abstracting the platform scan backend
//! - **Adapter**: [`NetshBssidScanner`] -- Tier 1 adapter that parses
//!   `netsh wlan show networks mode=bssid` output

pub mod adapter;
pub mod domain;
pub mod error;
pub mod pipeline;
pub mod port;

// Re-export key types at the crate root for convenience.
pub use adapter::NetshBssidScanner;
pub use adapter::parse_netsh_output;
pub use adapter::WlanApiScanner;
pub use domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
pub use domain::frame::MultiApFrame;
pub use domain::registry::{BssidEntry, BssidMeta, BssidRegistry, RunningStats};
pub use domain::result::EnhancedSensingResult;
pub use error::WifiScanError;
pub use port::WlanScanPort;

#[cfg(feature = "pipeline")]
pub use pipeline::WindowsWifiPipeline;

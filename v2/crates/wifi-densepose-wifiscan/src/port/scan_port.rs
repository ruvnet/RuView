//! The primary port (driving side) for WiFi BSSID scanning.

use crate::domain::bssid::BssidObservation;
use crate::error::WifiScanError;

/// Port that abstracts the platform WiFi scanning backend.
///
/// Implementations include:
/// - [`crate::adapter::NetshBssidScanner`] -- Tier 1, subprocess-based.
/// - Future: `WlanApiBssidScanner` -- Tier 2, native FFI (feature-gated).
pub trait WlanScanPort: Send + Sync {
    /// Perform a scan and return all currently visible BSSIDs.
    fn scan(&self) -> Result<Vec<BssidObservation>, WifiScanError>;

    /// Return the BSSID to which the adapter is currently connected, if any.
    fn connected(&self) -> Result<Option<BssidObservation>, WifiScanError>;
}

//! Tier 2: Windows WLAN API adapter for higher scan rates.
//!
//! This module provides a higher-rate scanning interface that targets 10-20 Hz
//! scan rates compared to the Tier 1 [`NetshBssidScanner`]'s ~2 Hz limitation
//! (caused by subprocess spawn overhead per scan).
//!
//! # Current implementation
//!
//! The adapter currently wraps [`NetshBssidScanner`] and provides:
//!
//! - **Synchronous scanning** via [`WlanScanPort`] trait implementation
//! - **Async scanning** (feature-gated behind `"wlanapi"`) via
//!   `tokio::task::spawn_blocking`
//! - **Scan metrics** (count, timing) for performance monitoring
//! - **Rate estimation** based on observed inter-scan intervals
//!
//! # Future: native `wlanapi.dll` FFI
//!
//! When native WLAN API bindings are available, this adapter will call:
//!
//! - `WlanOpenHandle` -- open a session to the WLAN service
//! - `WlanEnumInterfaces` -- discover WLAN adapters
//! - `WlanScan` -- trigger a fresh scan
//! - `WlanGetNetworkBssList` -- retrieve raw BSS entries with RSSI
//! - `WlanCloseHandle` -- clean up the session handle
//!
//! This eliminates the `netsh.exe` process-spawn bottleneck and enables
//! true 10-20 Hz scan rates suitable for real-time sensing.
//!
//! # Platform
//!
//! Windows only. On other platforms this module is not compiled.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::adapter::netsh_scanner::NetshBssidScanner;
use crate::domain::bssid::BssidObservation;
use crate::error::WifiScanError;
use crate::port::WlanScanPort;

// ---------------------------------------------------------------------------
// Scan metrics
// ---------------------------------------------------------------------------

/// Accumulated metrics from scan operations.
#[derive(Debug, Clone)]
pub struct ScanMetrics {
    /// Total number of scans performed since creation.
    pub scan_count: u64,
    /// Total number of BSSIDs observed across all scans.
    pub total_bssids_observed: u64,
    /// Duration of the most recent scan.
    pub last_scan_duration: Option<Duration>,
    /// Estimated scan rate in Hz based on the last scan duration.
    /// Returns `None` if no scans have been performed yet.
    pub estimated_rate_hz: Option<f64>,
}

// ---------------------------------------------------------------------------
// WlanApiScanner
// ---------------------------------------------------------------------------

/// Tier 2 WLAN API scanner with async support and scan metrics.
///
/// Currently wraps [`NetshBssidScanner`] with performance instrumentation.
/// When native WLAN API bindings become available, the inner implementation
/// will switch to `WlanGetNetworkBssList` for approximately 10x higher scan
/// rates without changing the public interface.
///
/// # Example (sync)
///
/// ```no_run
/// use wifi_densepose_wifiscan::adapter::wlanapi_scanner::WlanApiScanner;
/// use wifi_densepose_wifiscan::port::WlanScanPort;
///
/// let scanner = WlanApiScanner::new();
/// let observations = scanner.scan().unwrap();
/// for obs in &observations {
///     println!("{}: {} dBm", obs.bssid, obs.rssi_dbm);
/// }
/// println!("metrics: {:?}", scanner.metrics());
/// ```
pub struct WlanApiScanner {
    /// The underlying Tier 1 scanner.
    inner: NetshBssidScanner,

    /// Number of scans performed.
    scan_count: AtomicU64,

    /// Total BSSIDs observed across all scans.
    total_bssids: AtomicU64,

    /// Timestamp of the most recent scan start (for rate estimation).
    ///
    /// Uses `std::sync::Mutex` because `Instant` is not atomic but we need
    /// interior mutability. The lock duration is negligible (one write per
    /// scan) so contention is not a concern.
    last_scan_start: std::sync::Mutex<Option<Instant>>,

    /// Duration of the most recent scan.
    last_scan_duration: std::sync::Mutex<Option<Duration>>,
}

impl WlanApiScanner {
    /// Create a new Tier 2 scanner.
    pub fn new() -> Self {
        Self {
            inner: NetshBssidScanner::new(),
            scan_count: AtomicU64::new(0),
            total_bssids: AtomicU64::new(0),
            last_scan_start: std::sync::Mutex::new(None),
            last_scan_duration: std::sync::Mutex::new(None),
        }
    }

    /// Return accumulated scan metrics.
    pub fn metrics(&self) -> ScanMetrics {
        let scan_count = self.scan_count.load(Ordering::Relaxed);
        let total_bssids_observed = self.total_bssids.load(Ordering::Relaxed);
        let last_scan_duration =
            *self.last_scan_duration.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let estimated_rate_hz = last_scan_duration.map(|d| {
            let secs = d.as_secs_f64();
            if secs > 0.0 {
                1.0 / secs
            } else {
                f64::INFINITY
            }
        });

        ScanMetrics {
            scan_count,
            total_bssids_observed,
            last_scan_duration,
            estimated_rate_hz,
        }
    }

    /// Return the number of scans performed so far.
    pub fn scan_count(&self) -> u64 {
        self.scan_count.load(Ordering::Relaxed)
    }

    /// Perform a synchronous scan with timing instrumentation.
    ///
    /// This is the core scan method that both the [`WlanScanPort`] trait
    /// implementation and the async wrapper delegate to.
    fn scan_instrumented(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let start = Instant::now();

        // Record scan start time.
        if let Ok(mut guard) = self.last_scan_start.lock() {
            *guard = Some(start);
        }

        // Delegate to the Tier 1 scanner.
        let results = self.inner.scan_sync()?;

        // Record metrics.
        let elapsed = start.elapsed();
        if let Ok(mut guard) = self.last_scan_duration.lock() {
            *guard = Some(elapsed);
        }

        self.scan_count.fetch_add(1, Ordering::Relaxed);
        self.total_bssids
            .fetch_add(results.len() as u64, Ordering::Relaxed);

        tracing::debug!(
            scan_count = self.scan_count.load(Ordering::Relaxed),
            bssid_count = results.len(),
            elapsed_ms = elapsed.as_millis(),
            "Tier 2 scan complete"
        );

        Ok(results)
    }

    /// Perform an async scan by offloading the blocking netsh call to
    /// a background thread.
    ///
    /// This is gated behind the `"wlanapi"` feature because it requires
    /// the `tokio` runtime dependency.
    ///
    /// # Errors
    ///
    /// Returns [`WifiScanError::ScanFailed`] if the background task panics
    /// or is cancelled, or propagates any error from the underlying scan.
    #[cfg(feature = "wlanapi")]
    pub async fn scan_async(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        // We need to create a fresh scanner for the blocking task because
        // `&self` is not `Send` across the spawn_blocking boundary.
        // `NetshBssidScanner` is cheap (zero-size struct) so this is fine.
        let inner = NetshBssidScanner::new();
        let start = Instant::now();

        let results = tokio::task::spawn_blocking(move || inner.scan_sync())
            .await
            .map_err(|e| WifiScanError::ScanFailed {
                reason: format!("async scan task failed: {e}"),
            })??;

        // Record metrics.
        let elapsed = start.elapsed();
        if let Ok(mut guard) = self.last_scan_duration.lock() {
            *guard = Some(elapsed);
        }
        self.scan_count.fetch_add(1, Ordering::Relaxed);
        self.total_bssids
            .fetch_add(results.len() as u64, Ordering::Relaxed);

        tracing::debug!(
            scan_count = self.scan_count.load(Ordering::Relaxed),
            bssid_count = results.len(),
            elapsed_ms = elapsed.as_millis(),
            "Tier 2 async scan complete"
        );

        Ok(results)
    }
}

impl Default for WlanApiScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WlanScanPort implementation (sync)
// ---------------------------------------------------------------------------

impl WlanScanPort for WlanApiScanner {
    fn scan(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        self.scan_instrumented()
    }

    fn connected(&self) -> Result<Option<BssidObservation>, WifiScanError> {
        // Not yet implemented for Tier 2 -- fall back to a full scan and
        // return the strongest signal (heuristic for "likely connected").
        let mut results = self.scan_instrumented()?;
        if results.is_empty() {
            return Ok(None);
        }
        // Sort by signal strength descending; return the strongest.
        results.sort_by(|a, b| {
            b.rssi_dbm
                .partial_cmp(&a.rssi_dbm)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(Some(results.swap_remove(0)))
    }
}

// ---------------------------------------------------------------------------
// Native WLAN API constants and frequency utilities
// ---------------------------------------------------------------------------

/// Native WLAN API constants and frequency conversion utilities.
///
/// When implemented, this will contain:
///
/// ```ignore
/// extern "system" {
///     fn WlanOpenHandle(
///         dwClientVersion: u32,
///         pReserved: *const std::ffi::c_void,
///         pdwNegotiatedVersion: *mut u32,
///         phClientHandle: *mut HANDLE,
///     ) -> u32;
///
///     fn WlanEnumInterfaces(
///         hClientHandle: HANDLE,
///         pReserved: *const std::ffi::c_void,
///         ppInterfaceList: *mut *mut WLAN_INTERFACE_INFO_LIST,
///     ) -> u32;
///
///     fn WlanGetNetworkBssList(
///         hClientHandle: HANDLE,
///         pInterfaceGuid: *const GUID,
///         pDot11Ssid: *const DOT11_SSID,
///         dot11BssType: DOT11_BSS_TYPE,
///         bSecurityEnabled: BOOL,
///         pReserved: *const std::ffi::c_void,
///         ppWlanBssList: *mut *mut WLAN_BSS_LIST,
///     ) -> u32;
///
///     fn WlanCloseHandle(
///         hClientHandle: HANDLE,
///         pReserved: *const std::ffi::c_void,
///     ) -> u32;
/// }
/// ```
///
/// The native API returns `WLAN_BSS_ENTRY` structs that include:
/// - `dot11Bssid` (6-byte MAC)
/// - `lRssi` (dBm as i32)
/// - `ulChCenterFrequency` (kHz, from which channel/band are derived)
/// - `dot11BssPhyType` (maps to `RadioType`)
///
/// This eliminates the netsh subprocess overhead entirely.
#[allow(dead_code)]
mod wlan_ffi {
    /// WLAN API client version 2 (Vista+).
    pub const WLAN_CLIENT_VERSION_2: u32 = 2;

    /// BSS type for infrastructure networks.
    pub const DOT11_BSS_TYPE_INFRASTRUCTURE: u32 = 1;

    /// Convert a center frequency in kHz to an 802.11 channel number.
    ///
    /// Covers 2.4 GHz (ch 1-14), 5 GHz (ch 36-177), and 6 GHz bands.
    #[allow(clippy::cast_possible_truncation)] // Channel numbers always fit in u8
    pub fn freq_khz_to_channel(frequency_khz: u32) -> u8 {
        let mhz = frequency_khz / 1000;
        match mhz {
            // 2.4 GHz band
            2412..=2472 => ((mhz - 2407) / 5) as u8,
            2484 => 14,
            // 5 GHz band
            5170..=5825 => ((mhz - 5000) / 5) as u8,
            // 6 GHz band (Wi-Fi 6E)
            5955..=7115 => ((mhz - 5950) / 5) as u8,
            _ => 0,
        }
    }

    /// Convert a center frequency in kHz to a band type discriminant.
    ///
    /// Returns 0 for 2.4 GHz, 1 for 5 GHz, 2 for 6 GHz.
    pub fn freq_khz_to_band(frequency_khz: u32) -> u8 {
        let mhz = frequency_khz / 1000;
        match mhz {
            5000..=5900 => 1, // 5 GHz
            5925..=7200 => 2, // 6 GHz
            _ => 0,           // 2.4 GHz and unknown
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- construction ---------------------------------------------------------

    #[test]
    fn new_creates_scanner_with_zero_metrics() {
        let scanner = WlanApiScanner::new();
        assert_eq!(scanner.scan_count(), 0);

        let m = scanner.metrics();
        assert_eq!(m.scan_count, 0);
        assert_eq!(m.total_bssids_observed, 0);
        assert!(m.last_scan_duration.is_none());
        assert!(m.estimated_rate_hz.is_none());
    }

    #[test]
    fn default_creates_scanner() {
        let scanner = WlanApiScanner::default();
        assert_eq!(scanner.scan_count(), 0);
    }

    // -- frequency conversion (FFI placeholder) --------------------------------

    #[test]
    fn freq_khz_to_channel_2_4ghz() {
        assert_eq!(wlan_ffi::freq_khz_to_channel(2_412_000), 1);
        assert_eq!(wlan_ffi::freq_khz_to_channel(2_437_000), 6);
        assert_eq!(wlan_ffi::freq_khz_to_channel(2_462_000), 11);
        assert_eq!(wlan_ffi::freq_khz_to_channel(2_484_000), 14);
    }

    #[test]
    fn freq_khz_to_channel_5ghz() {
        assert_eq!(wlan_ffi::freq_khz_to_channel(5_180_000), 36);
        assert_eq!(wlan_ffi::freq_khz_to_channel(5_240_000), 48);
        assert_eq!(wlan_ffi::freq_khz_to_channel(5_745_000), 149);
    }

    #[test]
    fn freq_khz_to_channel_6ghz() {
        // 6 GHz channel 1 = 5955 MHz
        assert_eq!(wlan_ffi::freq_khz_to_channel(5_955_000), 1);
        // 6 GHz channel 5 = 5975 MHz
        assert_eq!(wlan_ffi::freq_khz_to_channel(5_975_000), 5);
    }

    #[test]
    fn freq_khz_to_channel_unknown_returns_zero() {
        assert_eq!(wlan_ffi::freq_khz_to_channel(900_000), 0);
        assert_eq!(wlan_ffi::freq_khz_to_channel(0), 0);
    }

    #[test]
    fn freq_khz_to_band_classification() {
        assert_eq!(wlan_ffi::freq_khz_to_band(2_437_000), 0); // 2.4 GHz
        assert_eq!(wlan_ffi::freq_khz_to_band(5_180_000), 1); // 5 GHz
        assert_eq!(wlan_ffi::freq_khz_to_band(5_975_000), 2); // 6 GHz
    }

    // -- WlanScanPort trait compliance -----------------------------------------

    #[test]
    fn implements_wlan_scan_port() {
        // Compile-time check: WlanApiScanner implements WlanScanPort.
        fn assert_port<T: WlanScanPort>() {}
        assert_port::<WlanApiScanner>();
    }

    #[test]
    fn implements_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WlanApiScanner>();
    }

    // -- metrics structure -----------------------------------------------------

    #[test]
    fn scan_metrics_debug_display() {
        let m = ScanMetrics {
            scan_count: 42,
            total_bssids_observed: 126,
            last_scan_duration: Some(Duration::from_millis(150)),
            estimated_rate_hz: Some(1.0 / 0.15),
        };
        let debug = format!("{m:?}");
        assert!(debug.contains("42"));
        assert!(debug.contains("126"));
    }

    #[test]
    fn scan_metrics_clone() {
        let m = ScanMetrics {
            scan_count: 1,
            total_bssids_observed: 5,
            last_scan_duration: None,
            estimated_rate_hz: None,
        };
        let m2 = m.clone();
        assert_eq!(m2.scan_count, 1);
        assert_eq!(m2.total_bssids_observed, 5);
    }

    // -- rate estimation -------------------------------------------------------

    #[test]
    fn estimated_rate_from_known_duration() {
        let scanner = WlanApiScanner::new();

        // Manually set last_scan_duration to simulate a completed scan.
        {
            let mut guard = scanner.last_scan_duration.lock().unwrap();
            *guard = Some(Duration::from_millis(100));
        }

        let m = scanner.metrics();
        let rate = m.estimated_rate_hz.unwrap();
        // 100ms per scan => 10 Hz
        assert!((rate - 10.0).abs() < 0.01, "expected ~10 Hz, got {rate}");
    }

    #[test]
    fn estimated_rate_none_before_first_scan() {
        let scanner = WlanApiScanner::new();
        assert!(scanner.metrics().estimated_rate_hz.is_none());
    }
}

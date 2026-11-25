//! Tier 2: Windows WLAN API adapter with a native `wlanapi.dll` scan path.
//!
//! This adapter prefers the **native** [`wlanapi_native::scan_native`] FFI
//! (`WlanOpenHandle` → `WlanEnumInterfaces` → `WlanGetNetworkBssList`),
//! which reads the driver's cached BSS list with no `netsh.exe`
//! subprocess. The native read path is bounded by WLAN-service IPC rather
//! than a `CreateProcess` per scan (the Tier 1 [`NetshBssidScanner`]'s
//! ~2 Hz ceiling), so polling it in a loop can observe BSSID updates
//! faster. The exact achieved rate is **measured** by
//! [`WlanApiScanner::benchmark`] on the running machine, not assumed —
//! this module makes no fixed "10×" claim.
//!
//! When the native path is unavailable (non-Windows, or the WLAN service
//! returns an error) the adapter transparently falls back to the
//! documented `netsh` Tier 1 scanner, so callers always get a result on
//! Windows and a typed [`WifiScanError::Unsupported`] only where no
//! backend exists.
//!
//! # API
//!
//! - **Sync scan** via [`WlanScanPort`] (native-first, netsh fallback).
//! - **Native-only scan** via [`WlanApiScanner::scan_native`] (no
//!   fallback; surfaces the platform gate honestly).
//! - **Async scan** (`"wlanapi"` feature) via `tokio::task::spawn_blocking`.
//! - **Scan metrics** + **measured-rate benchmark**.
//!
//! # Platform
//!
//! Native FFI is Windows-only and lives in [`wlanapi_native`]; the rest of
//! this module compiles everywhere.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::adapter::netsh_scanner::NetshBssidScanner;
use crate::adapter::wlanapi_native;
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
    /// How many scans so far used the native FFI path (vs the netsh
    /// fallback). Lets callers verify the native path is actually live.
    pub native_scans: u64,
}

/// Outcome of a measured scan-rate benchmark — MEASURED, not claimed.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Number of scans actually executed.
    pub iterations: u32,
    /// Wall-clock time the benchmark took.
    pub total: Duration,
    /// Measured scans per second over the whole run.
    pub rate_hz: f64,
    /// Mean BSSIDs observed per scan.
    pub mean_bssids: f64,
    /// Which backend produced the samples.
    pub backend: ScanBackend,
}

/// Which backend serviced a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanBackend {
    /// Native `wlanapi.dll` BSS-list FFI.
    Native,
    /// `netsh wlan show networks` subprocess fallback.
    Netsh,
}

// ---------------------------------------------------------------------------
// WlanApiScanner
// ---------------------------------------------------------------------------

/// Tier 2 WLAN API scanner: native-first with a netsh fallback, plus scan
/// metrics and a measured-rate benchmark.
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
/// // Measure the REAL achieved rate on this machine (no hardcoded claim).
/// if let Ok(bench) = scanner.benchmark(20) {
///     println!("measured {:.1} Hz via {:?}", bench.rate_hz, bench.backend);
/// }
/// ```
pub struct WlanApiScanner {
    /// The underlying Tier 1 scanner (fallback path).
    inner: NetshBssidScanner,

    /// Number of scans performed.
    scan_count: AtomicU64,

    /// Total BSSIDs observed across all scans.
    total_bssids: AtomicU64,

    /// Number of scans serviced by the native FFI path.
    native_scans: AtomicU64,

    /// Timestamp of the most recent scan start (for rate estimation).
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
            native_scans: AtomicU64::new(0),
            last_scan_start: std::sync::Mutex::new(None),
            last_scan_duration: std::sync::Mutex::new(None),
        }
    }

    /// Create a Tier 2 scanner that prefers the native `wlanapi.dll` BSS
    /// list and transparently falls back to `netsh` when native is
    /// unavailable (non-Windows or WLAN-service error).
    ///
    /// This is the constructor `wifi_task` uses on Windows: one call site,
    /// native-first, no inline `netsh` repetition.
    pub fn native_first_with_netsh_fallback() -> Self {
        Self::new()
    }

    /// Return accumulated scan metrics.
    pub fn metrics(&self) -> ScanMetrics {
        let scan_count = self.scan_count.load(Ordering::Relaxed);
        let total_bssids_observed = self.total_bssids.load(Ordering::Relaxed);
        let native_scans = self.native_scans.load(Ordering::Relaxed);
        let last_scan_duration = *self
            .last_scan_duration
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            native_scans,
        }
    }

    /// Return the number of scans performed so far.
    pub fn scan_count(&self) -> u64 {
        self.scan_count.load(Ordering::Relaxed)
    }

    /// Number of scans serviced by the native `wlanapi.dll` FFI path.
    pub fn native_scan_count(&self) -> u64 {
        self.native_scans.load(Ordering::Relaxed)
    }

    /// Whether the native path is available on this build/platform.
    ///
    /// `true` on Windows (FFI compiled), `false` elsewhere. Honest report
    /// of the platform gate without performing a scan.
    pub fn native_available() -> bool {
        cfg!(windows)
    }

    /// Run one native-only scan with **no** netsh fallback.
    ///
    /// Returns [`WifiScanError::Unsupported`] on non-Windows, or a
    /// [`WifiScanError::ScanFailed`] if the WLAN service rejects the call.
    /// Use this when a caller must know whether the native path worked.
    pub fn scan_native(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let start = Instant::now();
        let results = wlanapi_native::scan_native()?;
        self.record(start, results.len(), true);
        Ok(results)
    }

    /// Run one native scan and return only the **CSI-capable** APs.
    ///
    /// Filters the native BSS list to access points whose advertised PHY
    /// (HT/VHT/HE/EHT) supports channel sounding — the candidates usable as
    /// a CSI source. Honest about the platform gate: returns
    /// [`WifiScanError::Unsupported`] off-Windows.
    pub fn scan_native_csi_capable(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let all = self.scan_native()?;
        Ok(all
            .into_iter()
            .filter(|obs| wlanapi_native::is_csi_capable(obs.radio_type))
            .collect())
    }

    /// Record metrics for one completed scan.
    fn record(&self, start: Instant, bssid_count: usize, native: bool) {
        if let Ok(mut guard) = self.last_scan_start.lock() {
            *guard = Some(start);
        }
        let elapsed = start.elapsed();
        if let Ok(mut guard) = self.last_scan_duration.lock() {
            *guard = Some(elapsed);
        }
        self.scan_count.fetch_add(1, Ordering::Relaxed);
        self.total_bssids
            .fetch_add(bssid_count as u64, Ordering::Relaxed);
        if native {
            self.native_scans.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Perform a synchronous scan: native FFI first, netsh fallback.
    ///
    /// On Windows this attempts [`wlanapi_native::scan_native`]; if that
    /// errors (e.g. WLAN service unavailable) it falls back to the Tier 1
    /// netsh scanner. On non-Windows the native path returns `Unsupported`
    /// and the netsh fallback is used directly.
    fn scan_instrumented(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let start = Instant::now();

        match wlanapi_native::scan_native() {
            Ok(results) => {
                self.record(start, results.len(), true);
                tracing::debug!(
                    bssid_count = results.len(),
                    elapsed_ms = start.elapsed().as_millis(),
                    backend = "native",
                    "Tier 2 native scan complete"
                );
                Ok(results)
            }
            Err(native_err) => {
                tracing::debug!(%native_err, "native scan unavailable; falling back to netsh");
                let results = self.inner.scan_sync()?;
                self.record(start, results.len(), false);
                tracing::debug!(
                    bssid_count = results.len(),
                    elapsed_ms = start.elapsed().as_millis(),
                    backend = "netsh",
                    "Tier 2 netsh fallback scan complete"
                );
                Ok(results)
            }
        }
    }

    /// Measure the **real** achieved scan rate over `iterations` scans.
    ///
    /// This is the honest answer to "how fast is the native path on this
    /// box": it runs `iterations` back-to-back scans, times the whole run,
    /// and reports scans/second. No rate is hardcoded or extrapolated. The
    /// reported [`ScanBackend`] tells you whether the samples came from the
    /// native FFI or the netsh fallback.
    ///
    /// # Errors
    ///
    /// Propagates the first scan error; returns
    /// [`WifiScanError::ScanFailed`] if `iterations` is 0.
    pub fn benchmark(&self, iterations: u32) -> Result<BenchmarkResult, WifiScanError> {
        if iterations == 0 {
            return Err(WifiScanError::ScanFailed {
                reason: "benchmark requires iterations >= 1".to_string(),
            });
        }

        // Decide the backend once up front so the measurement is single-path.
        let native_first = wlanapi_native::scan_native();
        let (backend, mut total_bssids, mut done) = match &native_first {
            Ok(list) => (ScanBackend::Native, list.len() as u64, 1u32),
            Err(_) => (ScanBackend::Netsh, 0u64, 0u32),
        };

        let start = Instant::now();
        while done < iterations {
            let list = match backend {
                ScanBackend::Native => wlanapi_native::scan_native()?,
                ScanBackend::Netsh => self.inner.scan_sync()?,
            };
            total_bssids += list.len() as u64;
            done += 1;
        }
        let total = start.elapsed();
        let secs = total.as_secs_f64().max(f64::MIN_POSITIVE);

        Ok(BenchmarkResult {
            iterations,
            total,
            rate_hz: f64::from(iterations) / secs,
            mean_bssids: total_bssids as f64 / f64::from(iterations),
            backend,
        })
    }

    /// Measure the **real** achieved rate of a *specific* backend over a
    /// fixed wall-clock `window`, for an honest native-vs-netsh comparison.
    ///
    /// Unlike [`benchmark`](Self::benchmark) (which picks native-first and so
    /// never exercises netsh on a box where native works), this runs back-to-
    /// back scans on **exactly** the requested backend until `window` elapses,
    /// then reports the measured scans/second and mean BSSIDs/scan. This is the
    /// ADR-157 §5 #4 measurement primitive: drive it once per backend over the
    /// same window and compare the two `rate_hz` values — no rate is assumed.
    ///
    /// Returns `None` for [`ScanBackend::Native`] when the native path is
    /// unavailable (non-Windows or WLAN service error), so a caller can report
    /// the honest negative rather than a fabricated number.
    ///
    /// # Errors
    ///
    /// Propagates the first scan error from the chosen backend.
    pub fn benchmark_backend(
        &self,
        backend: ScanBackend,
        window: Duration,
    ) -> Result<Option<BenchmarkResult>, WifiScanError> {
        // Probe native availability first so an unavailable native path is an
        // honest `None`, not an error charged against the comparison.
        if backend == ScanBackend::Native && wlanapi_native::scan_native().is_err() {
            return Ok(None);
        }

        let start = Instant::now();
        let mut iterations: u32 = 0;
        let mut total_bssids: u64 = 0;
        while start.elapsed() < window {
            let list = match backend {
                ScanBackend::Native => wlanapi_native::scan_native()?,
                ScanBackend::Netsh => self.inner.scan_sync()?,
            };
            total_bssids += list.len() as u64;
            iterations += 1;
        }
        let total = start.elapsed();
        let secs = total.as_secs_f64().max(f64::MIN_POSITIVE);

        Ok(Some(BenchmarkResult {
            iterations,
            total,
            rate_hz: f64::from(iterations) / secs,
            mean_bssids: if iterations == 0 {
                0.0
            } else {
                total_bssids as f64 / f64::from(iterations)
            },
            backend,
        }))
    }

    /// Perform an async scan by offloading the blocking call to a
    /// background thread (native-first, netsh fallback inside the task).
    ///
    /// Gated behind the `"wlanapi"` feature (requires `tokio`).
    ///
    /// # Errors
    ///
    /// Returns [`WifiScanError::ScanFailed`] if the background task panics
    /// or is cancelled, or propagates any error from the underlying scan.
    #[cfg(feature = "wlanapi")]
    pub async fn scan_async(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let inner = NetshBssidScanner::new();
        let start = Instant::now();

        let (results, native) = tokio::task::spawn_blocking(
            move || -> Result<(Vec<BssidObservation>, bool), WifiScanError> {
                match wlanapi_native::scan_native() {
                    Ok(r) => Ok((r, true)),
                    Err(_) => Ok((inner.scan_sync()?, false)),
                }
            },
        )
        .await
        .map_err(|e| WifiScanError::ScanFailed {
            reason: format!("async scan task failed: {e}"),
        })??;

        self.record(start, results.len(), native);

        tracing::debug!(
            scan_count = self.scan_count.load(Ordering::Relaxed),
            bssid_count = results.len(),
            elapsed_ms = start.elapsed().as_millis(),
            native,
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
        // Heuristic: strongest visible BSSID is the likely-connected AP.
        let mut results = self.scan_instrumented()?;
        if results.is_empty() {
            return Ok(None);
        }
        results.sort_by(|a, b| {
            b.rssi_dbm
                .partial_cmp(&a.rssi_dbm)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(Some(results.swap_remove(0)))
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
        assert_eq!(scanner.native_scan_count(), 0);

        let m = scanner.metrics();
        assert_eq!(m.scan_count, 0);
        assert_eq!(m.total_bssids_observed, 0);
        assert_eq!(m.native_scans, 0);
        assert!(m.last_scan_duration.is_none());
        assert!(m.estimated_rate_hz.is_none());
    }

    #[test]
    fn default_creates_scanner() {
        let scanner = WlanApiScanner::default();
        assert_eq!(scanner.scan_count(), 0);
    }

    // -- native availability is an honest platform gate -----------------------

    #[test]
    fn native_available_matches_platform() {
        assert_eq!(WlanApiScanner::native_available(), cfg!(windows));
    }

    /// On non-Windows the native-only path must be a typed `Unsupported`.
    #[cfg(not(windows))]
    #[test]
    fn native_scan_unsupported_off_windows() {
        let scanner = WlanApiScanner::new();
        match scanner.scan_native() {
            Err(WifiScanError::Unsupported(_)) => {}
            other => panic!("expected Unsupported off-Windows, got {other:?}"),
        }
        // A failed native-only scan must not bump counters.
        assert_eq!(scanner.scan_count(), 0);
        assert_eq!(scanner.native_scan_count(), 0);
    }

    /// On Windows the native-only path runs the real FFI and, on success,
    /// records a native scan in the metrics.
    #[cfg(windows)]
    #[test]
    fn native_scan_records_metrics_on_windows() {
        let scanner = WlanApiScanner::new();
        match scanner.scan_native() {
            Ok(_) => {
                assert_eq!(scanner.native_scan_count(), 1);
                assert_eq!(scanner.scan_count(), 1);
            }
            // WLAN service off in CI is acceptable; just not Unsupported.
            Err(WifiScanError::ScanFailed { .. }) => {}
            Err(e) => panic!("unexpected native scan error on Windows: {e:?}"),
        }
    }

    // -- benchmark guards -----------------------------------------------------

    #[test]
    fn benchmark_rejects_zero_iterations() {
        let scanner = WlanApiScanner::new();
        assert!(matches!(
            scanner.benchmark(0),
            Err(WifiScanError::ScanFailed { .. })
        ));
    }

    // -- WlanScanPort trait compliance ----------------------------------------

    #[test]
    fn implements_wlan_scan_port() {
        fn assert_port<T: WlanScanPort>() {}
        assert_port::<WlanApiScanner>();
    }

    #[test]
    fn implements_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WlanApiScanner>();
    }

    // -- metrics structure ----------------------------------------------------

    #[test]
    fn scan_metrics_debug_display() {
        let m = ScanMetrics {
            scan_count: 42,
            total_bssids_observed: 126,
            last_scan_duration: Some(Duration::from_millis(150)),
            estimated_rate_hz: Some(1.0 / 0.15),
            native_scans: 40,
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
            native_scans: 1,
        };
        let m2 = m.clone();
        assert_eq!(m2.scan_count, 1);
        assert_eq!(m2.total_bssids_observed, 5);
        assert_eq!(m2.native_scans, 1);
    }

    #[test]
    fn benchmark_result_clone_and_fields() {
        let b = BenchmarkResult {
            iterations: 10,
            total: Duration::from_millis(500),
            rate_hz: 20.0,
            mean_bssids: 7.0,
            backend: ScanBackend::Native,
        };
        let b2 = b.clone();
        assert_eq!(b2.iterations, 10);
        assert_eq!(b2.backend, ScanBackend::Native);
        assert!((b2.rate_hz - 20.0).abs() < f64::EPSILON);
    }

    // -- rate estimation ------------------------------------------------------

    #[test]
    fn estimated_rate_from_known_duration() {
        let scanner = WlanApiScanner::new();
        {
            let mut guard = scanner.last_scan_duration.lock().unwrap();
            *guard = Some(Duration::from_millis(100));
        }
        let m = scanner.metrics();
        let rate = m.estimated_rate_hz.unwrap();
        assert!((rate - 10.0).abs() < 0.01, "expected ~10 Hz, got {rate}");
    }

    #[test]
    fn estimated_rate_none_before_first_scan() {
        let scanner = WlanApiScanner::new();
        assert!(scanner.metrics().estimated_rate_hz.is_none());
    }

    /// MEASURED scan-rate harness. `#[ignore]` so it never runs in CI (it
    /// touches the live WLAN service and takes seconds), but
    /// `cargo test -p wifi-densepose-wifiscan -- --ignored --nocapture
    /// measure_native_scan_rate` prints the *real* Hz on the running box.
    /// This is the honest measurement path: the number it prints is what
    /// the machine actually achieved, not a hardcoded claim.
    #[cfg(windows)]
    #[test]
    #[ignore = "live WLAN measurement; run explicitly with --ignored --nocapture"]
    fn measure_native_scan_rate() {
        let scanner = WlanApiScanner::new();
        let bench = scanner
            .benchmark(30)
            .expect("benchmark should run on a Windows box with a WLAN adapter");
        println!(
            "MEASURED native scan rate: {:.2} Hz over {} iters ({:?} backend), \
             mean {:.1} BSSIDs/scan, total {:?}",
            bench.rate_hz, bench.iterations, bench.backend, bench.mean_bssids, bench.total
        );
        assert!(bench.rate_hz > 0.0);
    }

    /// ADR-157 §5 #4 honest native-vs-netsh throughput comparison. `#[ignore]`
    /// (live WLAN, ~20 s). Run with:
    /// `cargo test -p wifi-densepose-wifiscan -- --ignored --nocapture
    /// measure_native_vs_netsh_throughput`. Drives BOTH backends over the same
    /// fixed wall-clock window and prints the measured Hz + BSSIDs/scan for
    /// each, plus the ratio — the real number, whatever it is (a null/negative
    /// result is a valid outcome and must be reported, not hidden).
    #[cfg(windows)]
    #[test]
    #[ignore = "live WLAN native-vs-netsh comparison; run with --ignored --nocapture"]
    fn measure_native_vs_netsh_throughput() {
        let scanner = WlanApiScanner::new();
        let window = Duration::from_secs(10);

        let native = scanner
            .benchmark_backend(ScanBackend::Native, window)
            .expect("native benchmark must not error");
        let netsh = scanner
            .benchmark_backend(ScanBackend::Netsh, window)
            .expect("netsh benchmark must not error")
            .expect("netsh is always available on Windows");

        match native {
            Some(n) => {
                println!(
                    "NATIVE: {:.2} Hz ({} scans / {:?}), mean {:.1} BSSIDs/scan",
                    n.rate_hz, n.iterations, n.total, n.mean_bssids
                );
                println!(
                    "NETSH:  {:.2} Hz ({} scans / {:?}), mean {:.1} BSSIDs/scan",
                    netsh.rate_hz, netsh.iterations, netsh.total, netsh.mean_bssids
                );
                let ratio = n.rate_hz / netsh.rate_hz.max(f64::MIN_POSITIVE);
                println!("RATIO native/netsh: {ratio:.2}x");
                assert!(n.rate_hz > 0.0 && netsh.rate_hz > 0.0);
            }
            None => {
                println!(
                    "NATIVE: unavailable on this box (WLAN service error). \
                     NETSH: {:.2} Hz, mean {:.1} BSSIDs/scan",
                    netsh.rate_hz, netsh.mean_bssids
                );
            }
        }
    }

    /// Determinism + handle-cleanup pin: N back-to-back native scans must all
    /// succeed (or all be the same typed error) with no resource exhaustion —
    /// a `WlanOpenHandle`/`WlanCloseHandle` leak would, after enough calls,
    /// surface as a `ScanFailed`. Running 50 iterations here exercises the
    /// open→enum→getlist→free→close cycle repeatedly. `#[ignore]` for CI (live
    /// WLAN service) but RUN on this box to verify no leak.
    #[cfg(windows)]
    #[test]
    #[ignore = "live WLAN handle-cleanup check; run with --ignored --nocapture"]
    fn native_scans_dont_leak_handles() {
        let scanner = WlanApiScanner::new();
        let mut ok = 0u32;
        let mut failed = 0u32;
        for _ in 0..50 {
            match scanner.scan_native() {
                Ok(_) => ok += 1,
                Err(WifiScanError::ScanFailed { .. }) => failed += 1,
                Err(e) => panic!("unexpected error during leak check: {e:?}"),
            }
        }
        println!("native leak check: {ok} ok, {failed} scan-failed of 50");
        // No leak ⇒ behavior is consistent across all 50 calls (all ok, or all
        // the same WLAN-service-off failure) — not a degrade partway through.
        assert!(ok == 50 || failed == 50, "inconsistent results suggest a leak: {ok} ok / {failed} failed");
    }
}

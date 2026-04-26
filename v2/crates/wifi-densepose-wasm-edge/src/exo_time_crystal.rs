//! Temporal symmetry breaking (time crystal) detector — ADR-041 exotic module.
//!
//! # Algorithm
//!
//! Samples `motion_energy` at frame rate (~20 Hz) into a 256-point circular
//! buffer.  Each frame computes the autocorrelation of the buffer at lags
//! 1..128 and searches for:
//!
//! 1. **Period doubling** -- a *discrete time translation symmetry breaking*
//!    signature.  Detected when the autocorrelation peak at lag L is strong
//!    (>0.5) AND the peak at lag 2L is also strong.  This mirrors the
//!    Floquet time-crystal criterion: the system oscillates at a sub-harmonic
//!    of the driving frequency.
//!
//! 2. **Multi-person temporal coordination** -- multiple autocorrelation peaks
//!    at non-harmonic ratios indicate coordinated but independent periodic
//!    motions (e.g., two people walking at different cadences).
//!
//! 3. **Stability** -- peak persistence is tracked across 10-second windows
//!    (200 frames at 20 Hz).  A crystal is "stable" only if the same
//!    period multiplier persists for the full window.
//!
//! # Events (680-series: Exotic / Research)
//!
//! - `CRYSTAL_DETECTED` (680): Period multiplier (2 = classic doubling).
//! - `CRYSTAL_STABILITY` (681): Stability score [0, 1] over the window.
//! - `COORDINATION_INDEX` (682): Number of distinct non-harmonic peaks.
//!
//! # Budget
//!
//! H (heavy, < 10 ms) -- autocorrelation of 256 points at 128 lags = 32K
//! multiply-accumulates, tight but within budget on ESP32-S3 WASM3.

use crate::vendor_common::{CircularBuffer, Ema};
use libm::fabsf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Motion energy circular buffer length (256 points at 20 Hz = 12.8 s).
const BUF_LEN: usize = 256;

/// Maximum autocorrelation lag to compute.
const MAX_LAG: usize = 128;

/// Minimum autocorrelation peak magnitude to count as "strong".
const PEAK_THRESHOLD: f32 = 0.5;

/// Minimum buffer fill before computing autocorrelation.
const MIN_FILL: usize = 64;

/// Ratio tolerance for harmonic detection: peaks within 5% of integer
/// multiples of the fundamental are considered harmonics, not independent.
const HARMONIC_TOLERANCE: f32 = 0.05;

/// Maximum number of distinct peaks to track for coordination index.
const MAX_PEAKS: usize = 8;

/// Stability window length in frames (10 s at 20 Hz).
const STABILITY_WINDOW: u32 = 200;

/// EMA smoothing factor for stability tracking.
const STABILITY_ALPHA: f32 = 0.05;

// ── Event IDs (680-series: Exotic) ───────────────────────────────────────────

pub const EVENT_CRYSTAL_DETECTED: i32 = 680;
pub const EVENT_CRYSTAL_STABILITY: i32 = 681;
pub const EVENT_COORDINATION_INDEX: i32 = 682;

// ── Time Crystal Detector ────────────────────────────────────────────────────

/// Temporal symmetry breaking pattern detector.
///
/// Samples `motion_energy` into a circular buffer and runs autocorrelation
/// to detect period doubling and multi-person temporal coordination.
pub struct TimeCrystalDetector {
    /// Circular buffer of motion energy samples.
    motion_buf: CircularBuffer<BUF_LEN>,
    /// Autocorrelation values at lags 1..MAX_LAG.
    autocorr: [f32; MAX_LAG],
    /// Last detected period multiplier (0 = none).
    last_multiplier: u8,
    /// Frame counter within the current stability window.
    stability_counter: u32,
    /// Number of frames in window where crystal was detected.
    stability_persist: u32,
    /// EMA-smoothed stability score [0, 1].
    stability_ema: Ema,
    /// Coordination index: count of distinct non-harmonic peaks.
    coordination: u8,
    /// Total frames processed.
    frame_count: u32,
    /// Whether crystal is currently detected.
    detected: bool,
    /// Cached buffer mean (for stats).
    buf_mean: f32,
    /// Cached buffer variance (for stats).
    buf_var: f32,
}

impl TimeCrystalDetector {
    pub const fn new() -> Self {
        Self {
            motion_buf: CircularBuffer::new(),
            autocorr: [0.0; MAX_LAG],
            last_multiplier: 0,
            stability_counter: 0,
            stability_persist: 0,
            stability_ema: Ema::new(STABILITY_ALPHA),
            coordination: 0,
            frame_count: 0,
            detected: false,
            buf_mean: 0.0,
            buf_var: 0.0,
        }
    }

    /// Process one frame.  `motion_energy` comes from the host Tier 2 DSP.
    ///
    /// Returns events as `(event_id, value)` pairs in a static buffer.
    pub fn process_frame(&mut self, motion_energy: f32) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_ev = 0usize;

        // Push sample into circular buffer.
        self.motion_buf.push(motion_energy);
        self.frame_count += 1;

        let fill = self.motion_buf.len();

        // Need at least MIN_FILL samples before analysis.
        if fill < MIN_FILL {
            return &[];
        }

        // Compute buffer statistics (mean, variance) for normalization.
        self.compute_stats(fill);

        // Skip if signal is essentially constant (no motion).
        if self.buf_var < 1e-8 {
            return &[];
        }

        // Compute normalized autocorrelation at lags 1..MAX_LAG.
        self.compute_autocorrelation(fill);

        // Find all local peaks in the autocorrelation.
        let max_lag = if fill / 2 < MAX_LAG { fill / 2 } else { MAX_LAG };

        let mut peak_lags = [0u16; MAX_PEAKS];
        let mut peak_vals = [0.0f32; MAX_PEAKS];
        let mut n_peaks = 0usize;

        // Skip trivial near-zero lags (start at lag 4).
        let mut i = 4;
        while i < max_lag.saturating_sub(1) {
            let prev = self.autocorr[i - 1];
            let curr = self.autocorr[i];
            let next = self.autocorr[i + 1];
            if curr > prev && curr > next && curr > PEAK_THRESHOLD {
                if n_peaks < MAX_PEAKS {
                    peak_lags[n_peaks] = (i + 1) as u16; // lag is 1-indexed
                    peak_vals[n_peaks] = curr;
                    n_peaks += 1;
                }
            }
            i += 1;
        }

        // Detect period doubling: peak at lag L AND peak at lag 2L.
        let mut detected_multiplier: u8 = 0;
        'outer: for p in 0..n_peaks {
            let lag_l = peak_lags[p] as usize;
            let lag_2l = lag_l * 2;
            if lag_2l > max_lag {
                continue;
            }
            // Check if there is a peak near lag 2L (+/- 2 tolerance).
            for q in 0..n_peaks {
                let lag_q = peak_lags[q] as usize;
                let diff = if lag_q > lag_2l {
                    lag_q - lag_2l
                } else {
                    lag_2l - lag_q
                };
                if diff <= 2 && peak_vals[q] > PEAK_THRESHOLD {
                    detected_multiplier = 2;
                    break 'outer;
                }
            }
        }

        // Count coordination index: number of distinct non-harmonic peaks.
        let coordination = self.count_non_harmonic_peaks(
            &peak_lags[..n_peaks],
        );
        self.coordination = coordination;
        self.detected = detected_multiplier > 0;

        // Update stability tracking.
        self.stability_counter += 1;
        if detected_multiplier > 0 && detected_multiplier == self.last_multiplier {
            self.stability_persist += 1;
        } else if detected_multiplier > 0 {
            self.stability_persist = 1;
        }

        if self.stability_counter >= STABILITY_WINDOW {
            let raw = self.stability_persist as f32 / STABILITY_WINDOW as f32;
            self.stability_ema.update(raw);
            self.stability_counter = 0;
            self.stability_persist = 0;
        }

        self.last_multiplier = detected_multiplier;

        // Emit events.
        if detected_multiplier > 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_CRYSTAL_DETECTED, detected_multiplier as f32);
            }
            n_ev += 1;
        }

        unsafe {
            EVENTS[n_ev] = (EVENT_CRYSTAL_STABILITY, self.stability_ema.value);
        }
        n_ev += 1;

        if coordination > 0 {
            unsafe {
                EVENTS[n_ev] = (EVENT_COORDINATION_INDEX, coordination as f32);
            }
            n_ev += 1;
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Compute mean and variance of the circular buffer contents.
    ///
    /// PERF: Single-pass computation using sum and sum-of-squares identity:
    ///   var = E[x^2] - E[x]^2 = (sum_sq / n) - (sum / n)^2
    /// Reduces from 2 passes (2 * fill get() calls with modulus) to 1 pass.
    fn compute_stats(&mut self, fill: usize) {
        let n = fill as f32;
        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;
        for i in 0..fill {
            let v = self.motion_buf.get(i);
            sum += v;
            sum_sq += v * v;
        }
        self.buf_mean = sum / n;
        // var = E[x^2] - (E[x])^2, clamped to avoid negative due to float rounding.
        let var = sum_sq / n - self.buf_mean * self.buf_mean;
        self.buf_var = if var > 0.0 { var } else { 0.0 };
    }

    /// Compute normalized autocorrelation r(k) for lags k=1..MAX_LAG.
    ///
    /// r(k) = (1/(N-k)) * sum_{t=0}^{N-k-1} (x[t]-mean)*(x[t+k]-mean) / var
    ///
    /// PERF: Pre-linearize circular buffer to contiguous stack array, eliminating
    /// modulus operations in the inner loop and improving cache locality.
    /// Reduces ~64K modulus ops to 0 for full buffer (256 * 128 * 2 get() calls).
    fn compute_autocorrelation(&mut self, fill: usize) {
        let max_lag = if fill / 2 < MAX_LAG { fill / 2 } else { MAX_LAG };
        let inv_var = 1.0 / self.buf_var;

        // Pre-linearize: copy circular buffer to contiguous array, subtracting
        // mean so we avoid the subtraction in the inner loop (saves fill*max_lag
        // subtractions).
        let mut linear = [0.0f32; BUF_LEN];
        for t in 0..fill {
            linear[t] = self.motion_buf.get(t) - self.buf_mean;
        }

        for k in 0..max_lag {
            let lag = k + 1; // lags 1..MAX_LAG
            let pairs = fill - lag;
            let mut sum = 0.0f32;
            // Inner loop now accesses contiguous memory with no modulus.
            let mut t = 0;
            while t < pairs {
                sum += linear[t] * linear[t + lag];
                t += 1;
            }
            self.autocorr[k] = (sum / pairs as f32) * inv_var;
        }

        // Zero out unused lags.
        for k in max_lag..MAX_LAG {
            self.autocorr[k] = 0.0;
        }
    }

    /// Count peaks whose lag ratios are not integer multiples of any other
    /// peak's lag.  These represent independent periodic components.
    fn count_non_harmonic_peaks(&self, lags: &[u16]) -> u8 {
        if lags.is_empty() {
            return 0;
        }
        if lags.len() == 1 {
            return 1;
        }

        let fundamental = lags[0] as f32;
        if fundamental < 1.0 {
            return lags.len() as u8;
        }
        let mut independent = 1u8; // fundamental itself counts

        for i in 1..lags.len() {
            let ratio = lags[i] as f32 / fundamental;
            let nearest_int = (ratio + 0.5) as u32;
            if nearest_int == 0 {
                independent += 1;
                continue;
            }
            let deviation = fabsf(ratio - nearest_int as f32) / nearest_int as f32;
            if deviation > HARMONIC_TOLERANCE {
                independent += 1;
            }
        }
        independent
    }

    /// Get the most recent autocorrelation values.
    pub fn autocorrelation(&self) -> &[f32; MAX_LAG] {
        &self.autocorr
    }

    /// Get the current stability score [0, 1].
    pub fn stability(&self) -> f32 {
        self.stability_ema.value
    }

    /// Get the last detected period multiplier (0 = none, 2 = doubling).
    pub fn multiplier(&self) -> u8 {
        self.last_multiplier
    }

    /// Whether a crystal pattern is currently detected.
    pub fn is_detected(&self) -> bool {
        self.detected
    }

    /// Get the coordination index (non-harmonic peak count).
    pub fn coordination_index(&self) -> u8 {
        self.coordination
    }

    /// Total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_new() {
        let tc = TimeCrystalDetector::new();
        assert_eq!(tc.frame_count(), 0);
        assert_eq!(tc.multiplier(), 0);
        assert_eq!(tc.coordination_index(), 0);
        assert!(!tc.is_detected());
    }

    #[test]
    fn test_insufficient_data_no_events() {
        let mut tc = TimeCrystalDetector::new();
        for i in 0..(MIN_FILL - 1) {
            let events = tc.process_frame(i as f32 * 0.1);
            assert!(events.is_empty(), "should not emit before MIN_FILL");
        }
    }

    #[test]
    fn test_constant_signal_no_crystal() {
        let mut tc = TimeCrystalDetector::new();
        for _ in 0..BUF_LEN {
            let events = tc.process_frame(1.0);
            for ev in events {
                assert_ne!(ev.0, EVENT_CRYSTAL_DETECTED,
                    "constant signal should not produce crystal");
            }
        }
    }

    #[test]
    fn test_periodic_signal_produces_autocorrelation_peak() {
        let mut tc = TimeCrystalDetector::new();
        // Generate a periodic signal: period = 10 frames.
        for frame in 0..BUF_LEN {
            let val = if (frame % 10) < 5 { 1.0 } else { 0.0 };
            tc.process_frame(val);
        }
        // The autocorrelation at lag 10 should be near 1.0.
        let acorr_lag10 = tc.autocorrelation()[9]; // 0-indexed: autocorr[k] is lag k+1
        assert!(acorr_lag10 > 0.5,
            "periodic signal should have strong autocorrelation at period lag, got {}",
            acorr_lag10);
    }

    #[test]
    fn test_coordination_single_peak() {
        let tc = TimeCrystalDetector::new();
        let lags = [10u16];
        let coord = tc.count_non_harmonic_peaks(&lags);
        assert_eq!(coord, 1, "single peak = 1 independent component");
    }

    #[test]
    fn test_coordination_harmonic_peaks() {
        let tc = TimeCrystalDetector::new();
        let lags = [10u16, 20, 30];
        let coord = tc.count_non_harmonic_peaks(&lags);
        assert_eq!(coord, 1, "harmonics of fundamental should count as 1");
    }

    #[test]
    fn test_coordination_non_harmonic_peaks() {
        let tc = TimeCrystalDetector::new();
        let lags = [10u16, 17];
        let coord = tc.count_non_harmonic_peaks(&lags);
        assert_eq!(coord, 2, "non-harmonic peak should count as independent");
    }

    #[test]
    fn test_reset() {
        let mut tc = TimeCrystalDetector::new();
        for _ in 0..100 {
            tc.process_frame(1.5);
        }
        assert!(tc.frame_count() > 0);
        tc.reset();
        assert_eq!(tc.frame_count(), 0);
        assert_eq!(tc.multiplier(), 0);
    }
}

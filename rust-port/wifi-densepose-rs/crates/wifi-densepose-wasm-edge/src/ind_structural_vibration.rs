//! Structural vibration monitoring — ADR-041 Category 5 Industrial module.
//!
//! Uses CSI phase stability to detect building vibration, seismic activity,
//! and structural stress in unoccupied spaces.
//!
//! When no humans are present, CSI phase should be highly stable (~0.02 rad
//! noise floor). Deviations from this baseline indicate structural events:
//!
//! - **Seismic**: broadband energy increase (>1 Hz), affects all subcarriers
//! - **Mechanical resonance**: narrowband harmonics, periodic in specific
//!   subcarrier groups
//! - **Structural drift**: slow monotonic phase change over minutes, indicating
//!   material stress or thermal expansion
//!
//! Maintains a vibration spectral density estimate via autocorrelation.
//!
//! Budget: H (<10 ms per frame).  Event IDs 540-543.

use libm::fabsf;
#[cfg(not(feature = "std"))]
use libm::sqrtf;
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// Phase history depth for spectral analysis (2 seconds at 20 Hz).
const PHASE_HISTORY_LEN: usize = 40;

/// Autocorrelation lags for spectral density estimation.
const MAX_LAGS: usize = 20;

/// Noise floor for phase (radians). Below this, no vibration.
const PHASE_NOISE_FLOOR: f32 = 0.02;

/// Seismic detection threshold: broadband RMS above noise floor.
const SEISMIC_THRESH: f32 = 0.15;

/// Mechanical resonance threshold: peak-to-mean ratio in autocorrelation.
const RESONANCE_PEAK_RATIO: f32 = 3.0;

/// Structural drift threshold (rad/frame, monotonic).
const DRIFT_RATE_THRESH: f32 = 0.0005;

/// Minimum drift duration (frames) before alerting (30 seconds at 20 Hz).
const DRIFT_MIN_FRAMES: u32 = 600;

/// Debounce frames for seismic detection.
const SEISMIC_DEBOUNCE: u8 = 4;

/// Debounce frames for resonance detection.
const RESONANCE_DEBOUNCE: u8 = 6;

/// Cooldown frames after seismic alert.
const SEISMIC_COOLDOWN: u16 = 200;

/// Cooldown frames after resonance alert.
const RESONANCE_COOLDOWN: u16 = 200;

/// Cooldown frames after drift alert.
const DRIFT_COOLDOWN: u16 = 600;

/// Spectrum report interval (frames, ~5 seconds).
const SPECTRUM_REPORT_INTERVAL: u32 = 100;

/// Event IDs (540-series: Industrial/Structural).
pub const EVENT_SEISMIC_DETECTED: i32 = 540;
pub const EVENT_MECHANICAL_RESONANCE: i32 = 541;
pub const EVENT_STRUCTURAL_DRIFT: i32 = 542;
pub const EVENT_VIBRATION_SPECTRUM: i32 = 543;

/// Structural vibration monitor.
pub struct StructuralVibrationMonitor {
    /// Phase history ring buffer [time][subcarrier].
    phase_history: [[f32; MAX_SC]; PHASE_HISTORY_LEN],
    hist_idx: usize,
    hist_len: usize,
    /// Baseline phase (calibrated when no humans present).
    baseline_phase: [f32; MAX_SC],
    baseline_set: bool,
    /// Drift tracking: accumulated phase per subcarrier.
    drift_accumulator: [f32; MAX_SC],
    drift_direction: [i8; MAX_SC], // +1 increasing, -1 decreasing, 0 unknown
    drift_frames: u32,
    /// Debounce counters.
    seismic_debounce: u8,
    resonance_debounce: u8,
    /// Cooldowns.
    seismic_cooldown: u16,
    resonance_cooldown: u16,
    drift_cooldown: u16,
    /// Frame counter.
    frame_count: u32,
    /// Calibration accumulator.
    calib_phase_sum: [f32; MAX_SC],
    calib_count: u32,
    /// Most recent RMS vibration level.
    last_rms: f32,
    /// Most recent dominant frequency bin (autocorrelation lag).
    last_dominant_lag: usize,
}

impl StructuralVibrationMonitor {
    pub const fn new() -> Self {
        Self {
            phase_history: [[0.0; MAX_SC]; PHASE_HISTORY_LEN],
            hist_idx: 0,
            hist_len: 0,
            baseline_phase: [0.0; MAX_SC],
            baseline_set: false,
            drift_accumulator: [0.0; MAX_SC],
            drift_direction: [0i8; MAX_SC],
            drift_frames: 0,
            seismic_debounce: 0,
            resonance_debounce: 0,
            seismic_cooldown: 0,
            resonance_cooldown: 0,
            drift_cooldown: 0,
            frame_count: 0,
            calib_phase_sum: [0.0; MAX_SC],
            calib_count: 0,
            last_rms: 0.0,
            last_dominant_lag: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// # Arguments
    /// - `phases`: per-subcarrier phase values
    /// - `amplitudes`: per-subcarrier amplitude values
    /// - `variance`: per-subcarrier variance values
    /// - `presence`: host-reported presence flag (0=empty, 1=occupied)
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        presence: i32,
    ) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(variance.len()).min(MAX_SC);
        if n_sc < 4 {
            return &[];
        }

        self.frame_count += 1;

        // Decrement cooldowns.
        if self.seismic_cooldown > 0 { self.seismic_cooldown -= 1; }
        if self.resonance_cooldown > 0 { self.resonance_cooldown -= 1; }
        if self.drift_cooldown > 0 { self.drift_cooldown -= 1; }

        // Store phase history.
        for i in 0..n_sc {
            self.phase_history[self.hist_idx][i] = phases[i];
        }
        self.hist_idx = (self.hist_idx + 1) % PHASE_HISTORY_LEN;
        if self.hist_len < PHASE_HISTORY_LEN {
            self.hist_len += 1;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        // --- Calibration: establish baseline when space is empty ---
        if !self.baseline_set {
            if presence == 0 {
                for i in 0..n_sc {
                    self.calib_phase_sum[i] += phases[i];
                }
                self.calib_count += 1;
                if self.calib_count >= 100 {
                    let n = self.calib_count as f32;
                    for i in 0..n_sc {
                        self.baseline_phase[i] = self.calib_phase_sum[i] / n;
                    }
                    self.baseline_set = true;
                }
            }
            return unsafe { &EVENTS[..0] };
        }

        // Only analyze when unoccupied (human presence masks structural signals).
        if presence > 0 {
            // Reset drift tracking when humans are present.
            self.drift_frames = 0;
            for i in 0..n_sc {
                self.drift_direction[i] = 0;
                self.drift_accumulator[i] = 0.0;
            }
            return unsafe { &EVENTS[..0] };
        }

        // --- Step 1: Compute phase deviation RMS ---
        let rms = self.compute_phase_rms(phases, n_sc);
        self.last_rms = rms;

        // --- Step 2: Seismic detection (broadband energy) ---
        if rms > SEISMIC_THRESH {
            // Check that energy is broadband: most subcarriers affected.
            let broadband = self.check_broadband(phases, n_sc);
            if broadband {
                self.seismic_debounce = self.seismic_debounce.saturating_add(1);
                if self.seismic_debounce >= SEISMIC_DEBOUNCE
                    && self.seismic_cooldown == 0
                    && n_events < 4
                {
                    self.seismic_cooldown = SEISMIC_COOLDOWN;
                    unsafe { EVENTS[n_events] = (EVENT_SEISMIC_DETECTED, rms); }
                    n_events += 1;
                }
            }
        } else {
            self.seismic_debounce = 0;
        }

        // --- Step 3: Mechanical resonance (narrowband peaks in autocorrelation) ---
        if self.hist_len >= PHASE_HISTORY_LEN {
            let (peak_ratio, dominant_lag) = self.compute_autocorrelation_peak(n_sc);
            self.last_dominant_lag = dominant_lag;

            if peak_ratio > RESONANCE_PEAK_RATIO && rms > PHASE_NOISE_FLOOR * 2.0 {
                self.resonance_debounce = self.resonance_debounce.saturating_add(1);
                if self.resonance_debounce >= RESONANCE_DEBOUNCE
                    && self.resonance_cooldown == 0
                    && n_events < 4
                {
                    self.resonance_cooldown = RESONANCE_COOLDOWN;
                    // Encode approximate frequency: 20 Hz / lag.
                    let freq = if dominant_lag > 0 {
                        20.0 / dominant_lag as f32
                    } else {
                        0.0
                    };
                    unsafe { EVENTS[n_events] = (EVENT_MECHANICAL_RESONANCE, freq); }
                    n_events += 1;
                }
            } else {
                self.resonance_debounce = 0;
            }
        }

        // --- Step 4: Structural drift (slow monotonic phase change) ---
        self.update_drift_tracking(phases, n_sc);
        if self.drift_frames >= DRIFT_MIN_FRAMES
            && self.drift_cooldown == 0
            && n_events < 4
        {
            let avg_drift = self.compute_average_drift(n_sc);
            if fabsf(avg_drift) > DRIFT_RATE_THRESH {
                self.drift_cooldown = DRIFT_COOLDOWN;
                // Value is drift rate in rad/second.
                unsafe { EVENTS[n_events] = (EVENT_STRUCTURAL_DRIFT, avg_drift * 20.0); }
                n_events += 1;
            }
        }

        // --- Step 5: Periodic vibration spectrum report ---
        if self.frame_count % SPECTRUM_REPORT_INTERVAL == 0
            && self.hist_len >= MAX_LAGS + 1
            && n_events < 4
        {
            unsafe { EVENTS[n_events] = (EVENT_VIBRATION_SPECTRUM, rms); }
            n_events += 1;
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Compute RMS phase deviation from baseline.
    fn compute_phase_rms(&self, phases: &[f32], n_sc: usize) -> f32 {
        let mut sum_sq = 0.0f32;
        for i in 0..n_sc {
            let d = phases[i] - self.baseline_phase[i];
            sum_sq += d * d;
        }
        sqrtf(sum_sq / n_sc as f32)
    }

    /// Check if phase disturbance is broadband (>60% of subcarriers affected).
    fn check_broadband(&self, phases: &[f32], n_sc: usize) -> bool {
        let mut affected = 0u32;
        for i in 0..n_sc {
            let d = fabsf(phases[i] - self.baseline_phase[i]);
            if d > PHASE_NOISE_FLOOR * 3.0 {
                affected += 1;
            }
        }
        (affected as f32 / n_sc as f32) > 0.6
    }

    /// Compute autocorrelation peak ratio and dominant lag.
    ///
    /// Returns (peak_to_mean_ratio, lag_of_peak).
    /// Uses the mean phase across subcarriers for the temporal signal.
    fn compute_autocorrelation_peak(&self, n_sc: usize) -> (f32, usize) {
        // Extract mean phase time series.
        let mut signal = [0.0f32; PHASE_HISTORY_LEN];
        for t in 0..self.hist_len {
            let idx = (self.hist_idx + PHASE_HISTORY_LEN - self.hist_len + t)
                % PHASE_HISTORY_LEN;
            let mut mean = 0.0f32;
            for sc in 0..n_sc {
                mean += self.phase_history[idx][sc];
            }
            signal[t] = mean / n_sc as f32;
        }

        // Subtract mean.
        let mut sig_mean = 0.0f32;
        for t in 0..self.hist_len {
            sig_mean += signal[t];
        }
        sig_mean /= self.hist_len as f32;
        for t in 0..self.hist_len {
            signal[t] -= sig_mean;
        }

        // Compute autocorrelation for lags 1..MAX_LAGS.
        let mut autocorr = [0.0f32; MAX_LAGS];
        let mut r0 = 0.0f32;
        for t in 0..self.hist_len {
            r0 += signal[t] * signal[t];
        }

        if r0 < 1e-10 {
            return (0.0, 0);
        }

        let mut peak_val = 0.0f32;
        let mut peak_lag = 1usize;
        let mut acorr_sum = 0.0f32;

        for lag in 1..MAX_LAGS.min(self.hist_len) {
            let mut r = 0.0f32;
            for t in 0..(self.hist_len - lag) {
                r += signal[t] * signal[t + lag];
            }
            let normalized = r / r0;
            autocorr[lag] = normalized;
            acorr_sum += fabsf(normalized);

            if fabsf(normalized) > fabsf(peak_val) {
                peak_val = normalized;
                peak_lag = lag;
            }
        }

        let n_lags = (MAX_LAGS.min(self.hist_len) - 1) as f32;
        let mean_acorr = if n_lags > 0.0 { acorr_sum / n_lags } else { 0.001 };

        let ratio = if mean_acorr > 0.001 {
            fabsf(peak_val) / mean_acorr
        } else {
            0.0
        };

        (ratio, peak_lag)
    }

    /// Update drift tracking: detect slow monotonic phase changes.
    fn update_drift_tracking(&mut self, phases: &[f32], n_sc: usize) {
        let mut consistent_drift = 0u32;

        for i in 0..n_sc {
            let delta = phases[i] - self.baseline_phase[i] - self.drift_accumulator[i];
            self.drift_accumulator[i] = phases[i] - self.baseline_phase[i];

            let new_dir = if delta > DRIFT_RATE_THRESH {
                1i8
            } else if delta < -DRIFT_RATE_THRESH {
                -1i8
            } else {
                self.drift_direction[i]
            };

            if new_dir == self.drift_direction[i] && new_dir != 0 {
                consistent_drift += 1;
            }
            self.drift_direction[i] = new_dir;
        }

        // If >50% of subcarriers show consistent drift direction.
        if (consistent_drift as f32 / n_sc as f32) > 0.5 {
            self.drift_frames += 1;
        } else {
            self.drift_frames = 0;
        }
    }

    /// Compute average drift rate across subcarriers (rad/frame).
    fn compute_average_drift(&self, n_sc: usize) -> f32 {
        if self.drift_frames == 0 || n_sc == 0 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        for i in 0..n_sc {
            sum += self.drift_accumulator[i];
        }
        sum / (n_sc as f32 * self.drift_frames as f32)
    }

    /// Current RMS vibration level.
    pub fn rms_vibration(&self) -> f32 {
        self.last_rms
    }

    /// Whether baseline has been established.
    pub fn is_calibrated(&self) -> bool {
        self.baseline_set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_calibrated_monitor() -> StructuralVibrationMonitor {
        let mut mon = StructuralVibrationMonitor::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        // Calibrate with 100 empty frames.
        for _ in 0..100 {
            mon.process_frame(&phases, &amps, &var, 0);
        }
        assert!(mon.is_calibrated());
        mon
    }

    #[test]
    fn test_init_state() {
        let mon = StructuralVibrationMonitor::new();
        assert!(!mon.is_calibrated());
        assert!((mon.rms_vibration() - 0.0).abs() < 0.01);
        assert_eq!(mon.frame_count, 0);
    }

    #[test]
    fn test_calibration() {
        let mut mon = StructuralVibrationMonitor::new();
        let phases = [0.5f32; 16];
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        for _ in 0..99 {
            mon.process_frame(&phases, &amps, &var, 0);
        }
        assert!(!mon.is_calibrated());

        mon.process_frame(&phases, &amps, &var, 0);
        assert!(mon.is_calibrated());
        // Baseline should be ~0.5.
        assert!((mon.baseline_phase[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_quiet_no_events() {
        let mut mon = make_calibrated_monitor();
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        // Feed stable phases (at baseline) — should produce no alerts.
        let phases = [0.0f32; 16];
        for _ in 0..200 {
            let events = mon.process_frame(&phases, &amps, &var, 0);
            for &(et, _) in events {
                assert!(
                    et != EVENT_SEISMIC_DETECTED && et != EVENT_MECHANICAL_RESONANCE,
                    "no alerts expected on quiet signal"
                );
            }
        }
        assert!(mon.rms_vibration() < PHASE_NOISE_FLOOR);
    }

    #[test]
    fn test_seismic_detection() {
        let mut mon = make_calibrated_monitor();
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        // Inject broadband phase disturbance.
        let mut seismic_detected = false;
        for frame in 0..20 {
            let phase_val = 0.5 * ((frame as f32) * 0.7).sin(); // large broadband
            let phases = [phase_val; 16]; // affects all subcarriers
            let events = mon.process_frame(&phases, &amps, &var, 0);
            for &(et, _) in events {
                if et == EVENT_SEISMIC_DETECTED {
                    seismic_detected = true;
                }
            }
        }

        assert!(seismic_detected, "seismic event should be detected with broadband disturbance");
    }

    #[test]
    fn test_no_events_when_occupied() {
        let mut mon = make_calibrated_monitor();
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        // Large disturbance but presence=1 => no structural alerts.
        let phases = [1.0f32; 16];
        for _ in 0..50 {
            let events = mon.process_frame(&phases, &amps, &var, 1);
            assert!(events.is_empty(), "no events when humans are present");
        }
    }

    #[test]
    fn test_vibration_spectrum_report() {
        let mut mon = make_calibrated_monitor();
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        let mut spectrum_reported = false;
        // Need enough history (PHASE_HISTORY_LEN frames) plus report interval.
        for frame in 0..200 {
            let phase_val = 0.01 * ((frame as f32) * 0.5).sin();
            let phases = [phase_val; 16];
            let events = mon.process_frame(&phases, &amps, &var, 0);
            for &(et, _) in events {
                if et == EVENT_VIBRATION_SPECTRUM {
                    spectrum_reported = true;
                }
            }
        }

        assert!(spectrum_reported, "periodic vibration spectrum should be reported");
    }

    #[test]
    fn test_phase_rms_computation() {
        let mon = make_calibrated_monitor();
        // Baseline is [0.0; 16]. Phase of [0.1; 16] should give RMS = 0.1.
        let phases = [0.1f32; 16];
        let rms = mon.compute_phase_rms(&phases, 16);
        assert!((rms - 0.1).abs() < 0.01, "RMS should be ~0.1, got {}", rms);
    }

    #[test]
    fn test_broadband_check() {
        let mon = make_calibrated_monitor();
        // All subcarriers disturbed.
        let phases = [0.2f32; 16];
        assert!(mon.check_broadband(&phases, 16), "all subcarriers above threshold = broadband");

        // Only a few disturbed.
        let mut mixed = [0.0f32; 16];
        mixed[0] = 0.2;
        mixed[1] = 0.2;
        assert!(!mon.check_broadband(&mixed, 16), "few subcarriers disturbed = not broadband");
    }
}

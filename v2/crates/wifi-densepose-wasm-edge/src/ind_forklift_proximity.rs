//! Forklift/AGV proximity detection — ADR-041 Category 5 Industrial module.
//!
//! Detects dangerous proximity between pedestrians and forklifts/AGVs using
//! CSI signal characteristics:
//!
//! - **Forklift signature**: high-amplitude, low-frequency (<0.3 Hz) phase
//!   modulation combined with motor vibration harmonics.  Large metal bodies
//!   produce distinctive broadband amplitude increases.
//! - **Human signature**: moderate amplitude, higher-frequency (0.5-2 Hz)
//!   phase modulation from gait.
//! - **Co-occurrence alert**: When both signatures are simultaneously present,
//!   emit proximity warnings with distance category.
//!
//! Budget: S (<5 ms per frame).  Event IDs 500-502.

#[cfg(not(feature = "std"))]
use libm::sqrtf;
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// Phase history depth for frequency analysis (1 second at 20 Hz).
const PHASE_HISTORY: usize = 20;

/// Amplitude threshold ratio for forklift (large metal body).
/// Forklift amplitude is typically 2-5x baseline.
const FORKLIFT_AMP_RATIO: f32 = 2.5;

/// Motion energy threshold for human presence near vehicle.
const HUMAN_MOTION_THRESH: f32 = 0.15;

/// Low-frequency dominance ratio: fraction of energy below 0.3 Hz.
/// Forklifts have >60% of energy in low frequencies.
const LOW_FREQ_RATIO_THRESH: f32 = 0.55;

/// Variance threshold for motor vibration harmonics.
const VIBRATION_VAR_THRESH: f32 = 0.08;

/// Debounce frames before emitting vehicle detection.
const VEHICLE_DEBOUNCE: u8 = 4;

/// Debounce frames before emitting proximity alert.
const PROXIMITY_DEBOUNCE: u8 = 2;

/// Cooldown frames after proximity alert.
const ALERT_COOLDOWN: u16 = 40;

/// Distance categories based on signal strength.
const DIST_CRITICAL: f32 = 4.0;   // amplitude ratio > 4.0 = very close
const DIST_WARNING: f32 = 3.0;    // amplitude ratio > 3.0 = close
// Below WARNING = caution

/// Event IDs (500-series: Industrial).
pub const EVENT_PROXIMITY_WARNING: i32 = 500;
pub const EVENT_VEHICLE_DETECTED: i32 = 501;
pub const EVENT_HUMAN_NEAR_VEHICLE: i32 = 502;

/// Forklift proximity detector.
pub struct ForkliftProximityDetector {
    /// Per-subcarrier baseline amplitude (calibrated).
    baseline_amp: [f32; MAX_SC],
    /// Phase history ring buffer for frequency analysis.
    phase_history: [[f32; MAX_SC]; PHASE_HISTORY],
    phase_hist_idx: usize,
    phase_hist_len: usize,
    /// Calibration state.
    calib_amp_sum: [f32; MAX_SC],
    calib_count: u32,
    calibrated: bool,
    /// Vehicle detection state.
    vehicle_present: bool,
    vehicle_debounce: u8,
    vehicle_amp_ratio: f32,
    /// Proximity alert state.
    proximity_debounce: u8,
    cooldown: u16,
    /// Frame counter.
    frame_count: u32,
}

impl ForkliftProximityDetector {
    pub const fn new() -> Self {
        Self {
            baseline_amp: [0.0; MAX_SC],
            phase_history: [[0.0; MAX_SC]; PHASE_HISTORY],
            phase_hist_idx: 0,
            phase_hist_len: 0,
            calib_amp_sum: [0.0; MAX_SC],
            calib_count: 0,
            calibrated: false,
            vehicle_present: false,
            vehicle_debounce: 0,
            vehicle_amp_ratio: 0.0,
            proximity_debounce: 0,
            cooldown: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame.
    ///
    /// # Arguments
    /// - `phases`: per-subcarrier phase values
    /// - `amplitudes`: per-subcarrier amplitude values
    /// - `variance`: per-subcarrier variance values
    /// - `motion_energy`: host-reported motion energy
    /// - `presence`: host-reported presence flag (0/1)
    /// - `n_persons`: host-reported person count
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        motion_energy: f32,
        presence: i32,
        n_persons: i32,
    ) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(variance.len()).min(MAX_SC);
        if n_sc < 4 {
            return &[];
        }

        self.frame_count += 1;

        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        // Store phase history.
        for i in 0..n_sc {
            self.phase_history[self.phase_hist_idx][i] = phases[i];
        }
        self.phase_hist_idx = (self.phase_hist_idx + 1) % PHASE_HISTORY;
        if self.phase_hist_len < PHASE_HISTORY {
            self.phase_hist_len += 1;
        }

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_events = 0usize;

        // Calibration phase: 100 frames (~5 seconds).
        if !self.calibrated {
            for i in 0..n_sc {
                self.calib_amp_sum[i] += amplitudes[i];
            }
            self.calib_count += 1;
            if self.calib_count >= 100 {
                let n = self.calib_count as f32;
                for i in 0..n_sc {
                    self.baseline_amp[i] = self.calib_amp_sum[i] / n;
                    if self.baseline_amp[i] < 0.01 {
                        self.baseline_amp[i] = 0.01;
                    }
                }
                self.calibrated = true;
            }
            return unsafe { &EVENTS[..0] };
        }

        // --- Step 1: Detect forklift/AGV signature ---
        let amp_ratio = self.compute_amplitude_ratio(amplitudes, n_sc);
        let low_freq_dominant = self.check_low_frequency_dominance(n_sc);
        let vibration_sig = self.compute_vibration_signature(variance, n_sc);

        let is_vehicle = amp_ratio > FORKLIFT_AMP_RATIO
            && low_freq_dominant
            && vibration_sig > VIBRATION_VAR_THRESH;

        if is_vehicle {
            self.vehicle_debounce = self.vehicle_debounce.saturating_add(1);
        } else {
            self.vehicle_debounce = self.vehicle_debounce.saturating_sub(1);
        }

        let was_vehicle = self.vehicle_present;
        self.vehicle_present = self.vehicle_debounce >= VEHICLE_DEBOUNCE;
        self.vehicle_amp_ratio = amp_ratio;

        // Emit vehicle detected on transition.
        if self.vehicle_present && !was_vehicle && n_events < 4 {
            unsafe {
                EVENTS[n_events] = (EVENT_VEHICLE_DETECTED, amp_ratio);
            }
            n_events += 1;
        }

        // --- Step 2: Check human presence near vehicle ---
        let human_present = (presence > 0 || n_persons > 0)
            && motion_energy > HUMAN_MOTION_THRESH;

        if self.vehicle_present && human_present {
            self.proximity_debounce = self.proximity_debounce.saturating_add(1);

            // Emit human-near-vehicle event on transition (debounce threshold reached).
            if self.proximity_debounce == PROXIMITY_DEBOUNCE && n_events < 4 {
                unsafe {
                    EVENTS[n_events] = (EVENT_HUMAN_NEAR_VEHICLE, motion_energy);
                }
                n_events += 1;
            }

            // Emit proximity warning with distance category.
            if self.proximity_debounce >= PROXIMITY_DEBOUNCE
                && self.cooldown == 0
                && n_events < 4
            {
                let dist_cat = if amp_ratio > DIST_CRITICAL {
                    0.0 // critical
                } else if amp_ratio > DIST_WARNING {
                    1.0 // warning
                } else {
                    2.0 // caution
                };
                unsafe {
                    EVENTS[n_events] = (EVENT_PROXIMITY_WARNING, dist_cat);
                }
                n_events += 1;
                self.cooldown = ALERT_COOLDOWN;
            }
        } else {
            self.proximity_debounce = 0;
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Compute mean amplitude ratio vs baseline across subcarriers.
    fn compute_amplitude_ratio(&self, amplitudes: &[f32], n_sc: usize) -> f32 {
        let mut ratio_sum = 0.0f32;
        let mut count = 0u32;
        for i in 0..n_sc {
            if self.baseline_amp[i] > 0.01 {
                ratio_sum += amplitudes[i] / self.baseline_amp[i];
                count += 1;
            }
        }
        if count == 0 { 1.0 } else { ratio_sum / count as f32 }
    }

    /// Check if phase modulation is dominated by low frequencies (<0.3 Hz).
    /// Uses simple energy ratio: variance of phase differences (proxy for
    /// high-frequency content) vs total phase variance.
    fn check_low_frequency_dominance(&self, n_sc: usize) -> bool {
        if self.phase_hist_len < 6 {
            return false;
        }

        // Compute total phase variance and high-frequency component.
        let mut total_var = 0.0f32;
        let mut hf_energy = 0.0f32;
        let mut count = 0u32;

        for sc in 0..n_sc.min(MAX_SC) {
            // Compute mean phase for this subcarrier.
            let mut sum = 0.0f32;
            for t in 0..self.phase_hist_len {
                let idx = (self.phase_hist_idx + PHASE_HISTORY - self.phase_hist_len + t) % PHASE_HISTORY;
                sum += self.phase_history[idx][sc];
            }
            let mean = sum / self.phase_hist_len as f32;

            // Total variance.
            let mut var = 0.0f32;
            for t in 0..self.phase_hist_len {
                let idx = (self.phase_hist_idx + PHASE_HISTORY - self.phase_hist_len + t) % PHASE_HISTORY;
                let d = self.phase_history[idx][sc] - mean;
                var += d * d;
            }
            total_var += var;

            // High-frequency: variance of first differences (approximates >1Hz).
            let mut diff_var = 0.0f32;
            for t in 1..self.phase_hist_len {
                let idx0 = (self.phase_hist_idx + PHASE_HISTORY - self.phase_hist_len + t - 1) % PHASE_HISTORY;
                let idx1 = (self.phase_hist_idx + PHASE_HISTORY - self.phase_hist_len + t) % PHASE_HISTORY;
                let d = self.phase_history[idx1][sc] - self.phase_history[idx0][sc];
                diff_var += d * d;
            }
            hf_energy += diff_var;
            count += 1;
        }

        if count == 0 || total_var < 0.001 {
            return false;
        }

        // Low frequency ratio: if high-freq energy is small relative to total.
        let lf_ratio = 1.0 - (hf_energy / (total_var + 0.001));
        lf_ratio > LOW_FREQ_RATIO_THRESH
    }

    /// Compute vibration signature from variance pattern.
    /// Motor vibration produces elevated, relatively uniform variance.
    fn compute_vibration_signature(&self, variance: &[f32], n_sc: usize) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..n_sc {
            sum += variance[i];
        }
        sum / n_sc as f32
    }

    /// Whether a vehicle is currently detected.
    pub fn is_vehicle_present(&self) -> bool {
        self.vehicle_present
    }

    /// Current amplitude ratio (proxy for vehicle proximity).
    pub fn amplitude_ratio(&self) -> f32 {
        self.vehicle_amp_ratio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_detector_calibrated() -> ForkliftProximityDetector {
        let mut det = ForkliftProximityDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];
        for _ in 0..100 {
            det.process_frame(&phases, &amps, &var, 0.0, 0, 0);
        }
        assert!(det.calibrated);
        det
    }

    #[test]
    fn test_init_state() {
        let det = ForkliftProximityDetector::new();
        assert!(!det.calibrated);
        assert!(!det.is_vehicle_present());
        assert_eq!(det.frame_count, 0);
    }

    #[test]
    fn test_calibration() {
        let mut det = ForkliftProximityDetector::new();
        let phases = [0.0f32; 16];
        let amps = [2.0f32; 16];
        let var = [0.01f32; 16];

        for _ in 0..99 {
            det.process_frame(&phases, &amps, &var, 0.0, 0, 0);
        }
        assert!(!det.calibrated);

        det.process_frame(&phases, &amps, &var, 0.0, 0, 0);
        assert!(det.calibrated);
        // Baseline should be ~2.0.
        assert!((det.baseline_amp[0] - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_no_alert_quiet_scene() {
        let mut det = make_detector_calibrated();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];
        let var = [0.01f32; 16];

        for _ in 0..50 {
            let events = det.process_frame(&phases, &amps, &var, 0.0, 0, 0);
            assert!(events.is_empty(), "no events expected in quiet scene");
        }
        assert!(!det.is_vehicle_present());
    }

    #[test]
    fn test_vehicle_detection() {
        let mut det = make_detector_calibrated();
        // Build up phase history first with slow-changing phases (low freq).
        let var_high = [0.12f32; 16];

        let mut vehicle_detected = false;
        for frame in 0..30 {
            // High amplitude + slow phase change + high variance = forklift.
            let phase_val = 0.1 * (frame as f32); // slow ramp => low frequency
            let phases = [phase_val; 16];
            let amps = [3.5f32; 16]; // 3.5x baseline of 1.0
            let events = det.process_frame(&phases, &amps, &var_high, 0.0, 0, 0);
            for &(et, _) in events {
                if et == EVENT_VEHICLE_DETECTED {
                    vehicle_detected = true;
                }
            }
        }
        assert!(vehicle_detected, "vehicle should be detected with high amp + low freq + vibration");
    }

    #[test]
    fn test_proximity_warning() {
        let mut det = make_detector_calibrated();
        let var_high = [0.12f32; 16];

        let mut proximity_warned = false;
        for frame in 0..40 {
            let phase_val = 0.1 * (frame as f32);
            let phases = [phase_val; 16];
            let amps = [4.5f32; 16]; // very high = critical distance
            // Human present + vehicle present => proximity warning.
            let events = det.process_frame(&phases, &amps, &var_high, 0.5, 1, 1);
            for &(et, val) in events {
                if et == EVENT_PROXIMITY_WARNING {
                    proximity_warned = true;
                    // Distance category 0 = critical (amp_ratio > 4.0).
                    assert!(val == 0.0 || val == 1.0 || val == 2.0);
                }
            }
        }
        assert!(proximity_warned, "proximity warning should fire when vehicle + human co-occur");
    }

    #[test]
    fn test_cooldown_prevents_flood() {
        let mut det = make_detector_calibrated();
        let var_high = [0.12f32; 16];

        let mut alert_count = 0u32;
        for frame in 0..100 {
            let phase_val = 0.1 * (frame as f32);
            let phases = [phase_val; 16];
            let amps = [4.0f32; 16];
            let events = det.process_frame(&phases, &amps, &var_high, 0.5, 1, 1);
            for &(et, _) in events {
                if et == EVENT_PROXIMITY_WARNING {
                    alert_count += 1;
                }
            }
        }
        // With ALERT_COOLDOWN=40, in 100 frames we should get at most ~3 alerts.
        assert!(alert_count <= 4, "cooldown should limit alert rate, got {}", alert_count);
    }

    #[test]
    fn test_amplitude_ratio_computation() {
        let det = make_detector_calibrated();
        // Baseline is 1.0, test with 3.0 amplitude.
        let amps = [3.0f32; 16];
        let ratio = det.compute_amplitude_ratio(&amps, 16);
        assert!((ratio - 3.0).abs() < 0.1, "amplitude ratio should be ~3.0, got {}", ratio);
    }
}

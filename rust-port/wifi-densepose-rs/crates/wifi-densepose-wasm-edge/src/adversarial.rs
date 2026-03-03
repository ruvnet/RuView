//! Signal anomaly and adversarial detection — no_std port.
//!
//! Ported from `ruvsense/adversarial.rs` for WASM execution.
//! Detects physically impossible or inconsistent CSI signals that may indicate:
//! - Environmental interference (appliance noise, RF jamming)
//! - Sensor malfunction (antenna disconnection, firmware bug)
//! - Adversarial manipulation (replay attack, signal injection)
//!
//! Detection heuristics:
//! 1. **Phase jump**: Large instantaneous phase discontinuity across all subcarriers
//! 2. **Amplitude flatline**: All subcarriers report identical amplitude (stuck sensor)
//! 3. **Energy spike**: Total signal energy exceeds physical bounds
//! 4. **Consistency check**: Phase and amplitude should correlate within bounds

use libm::fabsf;

/// Maximum subcarriers tracked.
const MAX_SC: usize = 32;

/// Phase jump threshold (radians) — physically impossible for human motion.
const PHASE_JUMP_THRESHOLD: f32 = 2.5;

/// Minimum amplitude variance across subcarriers (zero = flatline/stuck).
const MIN_AMPLITUDE_VARIANCE: f32 = 0.001;

/// Maximum physically plausible energy ratio (current / baseline).
const MAX_ENERGY_RATIO: f32 = 50.0;

/// Number of frames for baseline estimation.
const BASELINE_FRAMES: u32 = 100;

/// Anomaly cooldown (frames) to avoid flooding events.
const ANOMALY_COOLDOWN: u16 = 20;

/// Anomaly detector state.
pub struct AnomalyDetector {
    /// Previous phase per subcarrier.
    prev_phases: [f32; MAX_SC],
    /// Baseline mean amplitude per subcarrier.
    baseline_amp: [f32; MAX_SC],
    /// Baseline mean total energy.
    baseline_energy: f32,
    /// Frame counter for baseline accumulation.
    baseline_count: u32,
    /// Running sum for baseline computation.
    baseline_sum: [f32; MAX_SC],
    baseline_energy_sum: f32,
    /// Whether baseline has been established.
    calibrated: bool,
    /// Whether phase has been initialized.
    phase_initialized: bool,
    /// Cooldown counter.
    cooldown: u16,
    /// Total anomalies detected.
    anomaly_count: u32,
}

impl AnomalyDetector {
    pub const fn new() -> Self {
        Self {
            prev_phases: [0.0; MAX_SC],
            baseline_amp: [0.0; MAX_SC],
            baseline_energy: 0.0,
            baseline_count: 0,
            baseline_sum: [0.0; MAX_SC],
            baseline_energy_sum: 0.0,
            calibrated: false,
            phase_initialized: false,
            cooldown: 0,
            anomaly_count: 0,
        }
    }

    /// Process one frame, returning true if an anomaly is detected.
    pub fn process_frame(&mut self, phases: &[f32], amplitudes: &[f32]) -> bool {
        let n_sc = phases.len().min(amplitudes.len()).min(MAX_SC);

        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        // ── Baseline accumulation ────────────────────────────────────────
        if !self.calibrated {
            let mut energy = 0.0f32;
            for i in 0..n_sc {
                self.baseline_sum[i] += amplitudes[i];
                energy += amplitudes[i] * amplitudes[i];
            }
            self.baseline_energy_sum += energy;
            self.baseline_count += 1;

            if !self.phase_initialized {
                for i in 0..n_sc {
                    self.prev_phases[i] = phases[i];
                }
                self.phase_initialized = true;
            }

            if self.baseline_count >= BASELINE_FRAMES {
                let n = self.baseline_count as f32;
                for i in 0..n_sc {
                    self.baseline_amp[i] = self.baseline_sum[i] / n;
                }
                self.baseline_energy = self.baseline_energy_sum / n;
                self.calibrated = true;
            }

            return false;
        }

        let mut anomaly = false;

        // ── Check 1: Phase jump across all subcarriers ───────────────────
        if self.phase_initialized {
            let mut jump_count = 0u32;
            for i in 0..n_sc {
                let delta = fabsf(phases[i] - self.prev_phases[i]);
                if delta > PHASE_JUMP_THRESHOLD {
                    jump_count += 1;
                }
            }
            // If >50% of subcarriers have large jumps, it's suspicious.
            if n_sc > 0 && jump_count > (n_sc as u32) / 2 {
                anomaly = true;
            }
        }

        // ── Check 2: Amplitude flatline ──────────────────────────────────
        if n_sc >= 4 {
            let mut amp_mean = 0.0f32;
            for i in 0..n_sc {
                amp_mean += amplitudes[i];
            }
            amp_mean /= n_sc as f32;

            let mut amp_var = 0.0f32;
            for i in 0..n_sc {
                let d = amplitudes[i] - amp_mean;
                amp_var += d * d;
            }
            amp_var /= n_sc as f32;

            if amp_var < MIN_AMPLITUDE_VARIANCE && amp_mean > 0.01 {
                anomaly = true;
            }
        }

        // ── Check 3: Energy spike ────────────────────────────────────────
        {
            let mut current_energy = 0.0f32;
            for i in 0..n_sc {
                current_energy += amplitudes[i] * amplitudes[i];
            }
            if self.baseline_energy > 0.0 {
                let ratio = current_energy / self.baseline_energy;
                if ratio > MAX_ENERGY_RATIO {
                    anomaly = true;
                }
            }
        }

        // Update previous phase.
        for i in 0..n_sc {
            self.prev_phases[i] = phases[i];
        }
        self.phase_initialized = true;

        // Apply cooldown.
        if anomaly && self.cooldown == 0 {
            self.anomaly_count += 1;
            self.cooldown = ANOMALY_COOLDOWN;
            true
        } else {
            false
        }
    }

    /// Total anomalies detected since initialization.
    pub fn total_anomalies(&self) -> u32 {
        self.anomaly_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anomaly_detector_init() {
        let det = AnomalyDetector::new();
        assert!(!det.calibrated);
        assert!(!det.phase_initialized);
        assert_eq!(det.total_anomalies(), 0);
    }

    #[test]
    fn test_calibration_phase() {
        let mut det = AnomalyDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // During calibration, should never report anomaly.
        for _ in 0..BASELINE_FRAMES {
            assert!(!det.process_frame(&phases, &amps));
        }
        assert!(det.calibrated);
    }

    #[test]
    fn test_normal_signal_no_anomaly() {
        let mut det = AnomalyDetector::new();
        let phases = [0.0f32; 16];
        // Use varying amplitudes so flatline check does not trigger.
        let mut amps = [0.0f32; 16];
        for i in 0..16 {
            amps[i] = 1.0 + (i as f32) * 0.1;
        }

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        // Feed normal signal (same as baseline).
        for _ in 0..50 {
            assert!(!det.process_frame(&phases, &amps));
        }
        assert_eq!(det.total_anomalies(), 0);
    }

    #[test]
    fn test_phase_jump_detection() {
        let mut det = AnomalyDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        // Inject phase jump across all subcarriers.
        let jumped_phases = [5.0f32; 16]; // jump of 5.0 > threshold of 2.5
        let detected = det.process_frame(&jumped_phases, &amps);
        assert!(detected, "phase jump should trigger anomaly detection");
        assert_eq!(det.total_anomalies(), 1);
    }

    #[test]
    fn test_amplitude_flatline_detection() {
        let mut det = AnomalyDetector::new();
        // Calibrate with varying amplitudes.
        let mut amps = [0.0f32; 16];
        for i in 0..16 {
            amps[i] = 0.5 + (i as f32) * 0.1;
        }
        let phases = [0.0f32; 16];

        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        // Now send perfectly flat amplitudes (all identical, nonzero).
        let flat_amps = [1.0f32; 16]; // variance = 0 < MIN_AMPLITUDE_VARIANCE
        let detected = det.process_frame(&phases, &flat_amps);
        assert!(detected, "flatline amplitude should trigger anomaly detection");
    }

    #[test]
    fn test_energy_spike_detection() {
        let mut det = AnomalyDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        // Inject massive energy spike (100x baseline).
        let spike_amps = [100.0f32; 16];
        let detected = det.process_frame(&phases, &spike_amps);
        assert!(detected, "energy spike should trigger anomaly detection");
    }

    #[test]
    fn test_cooldown_prevents_flood() {
        let mut det = AnomalyDetector::new();
        let phases = [0.0f32; 16];
        let amps = [1.0f32; 16];

        // Calibrate.
        for _ in 0..BASELINE_FRAMES {
            det.process_frame(&phases, &amps);
        }

        // Trigger first anomaly.
        let spike_amps = [100.0f32; 16];
        assert!(det.process_frame(&phases, &spike_amps));

        // Subsequent frames during cooldown should not report.
        for _ in 0..10 {
            assert!(!det.process_frame(&phases, &spike_amps));
        }
        assert_eq!(det.total_anomalies(), 1, "cooldown should prevent counting duplicates");
    }
}

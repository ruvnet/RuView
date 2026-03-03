//! Concealed metallic object detection — ADR-041 Category 2 Security module.
//!
//! Detects concealed metallic objects via differential CSI multipath signatures.
//! Metal has significantly higher RF reflectivity than human tissue, producing
//! characteristic amplitude variance / phase variance ratios. This module is
//! research-grade and experimental — it requires calibration for each deployment
//! environment.
//!
//! The detection principle: when a person carrying a metallic object moves through
//! the sensing area, the multipath signature shows a higher amplitude-to-phase
//! variance ratio compared to a person without metal, because metal strongly
//! reflects RF energy while producing less phase dispersion than diffuse tissue.
//!
//! Events: METAL_ANOMALY(220), WEAPON_ALERT(221), CALIBRATION_NEEDED(222).
//! Budget: S (<5 ms).

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

const MAX_SC: usize = 32;
/// Calibration frames (5 seconds at 20 Hz).
const BASELINE_FRAMES: u32 = 100;
/// Amplitude variance / phase variance ratio threshold for metal detection.
const METAL_RATIO_THRESH: f32 = 4.0;
/// Elevated ratio for weapon-grade alert (very high reflectivity).
const WEAPON_RATIO_THRESH: f32 = 8.0;
/// Minimum motion energy to consider detection valid (ignore static scenes).
const MIN_MOTION_ENERGY: f32 = 0.5;
/// Minimum presence required (person must be present).
const MIN_PRESENCE: i32 = 1;
/// Consecutive frames for metal anomaly debounce.
const METAL_DEBOUNCE: u8 = 4;
/// Consecutive frames for weapon alert debounce.
const WEAPON_DEBOUNCE: u8 = 6;
/// Cooldown frames after event emission.
const COOLDOWN: u16 = 60;
/// Re-calibration trigger: if baseline drift exceeds this ratio.
const RECALIB_DRIFT_THRESH: f32 = 3.0;
/// Window for running variance computation.
const VAR_WINDOW: usize = 16;

pub const EVENT_METAL_ANOMALY: i32 = 220;
pub const EVENT_WEAPON_ALERT: i32 = 221;
pub const EVENT_CALIBRATION_NEEDED: i32 = 222;

/// Concealed metallic object detector.
pub struct WeaponDetector {
    /// Baseline amplitude variance per subcarrier.
    baseline_amp_var: [f32; MAX_SC],
    /// Baseline phase variance per subcarrier.
    baseline_phase_var: [f32; MAX_SC],
    /// Calibration: sum of amplitude values.
    cal_amp_sum: [f32; MAX_SC],
    cal_amp_sq_sum: [f32; MAX_SC],
    /// Calibration: sum of phase values.
    cal_phase_sum: [f32; MAX_SC],
    cal_phase_sq_sum: [f32; MAX_SC],
    cal_count: u32,
    calibrated: bool,
    /// Rolling amplitude window per subcarrier (flattened: MAX_SC * VAR_WINDOW).
    amp_window: [f32; MAX_SC],
    /// Rolling phase window per subcarrier.
    phase_window: [f32; MAX_SC],
    /// Running amplitude variance (Welford online).
    run_amp_mean: [f32; MAX_SC],
    run_amp_m2: [f32; MAX_SC],
    /// Running phase variance (Welford online).
    run_phase_mean: [f32; MAX_SC],
    run_phase_m2: [f32; MAX_SC],
    run_count: u32,
    /// Debounce counters.
    metal_run: u8,
    weapon_run: u8,
    /// Cooldowns.
    cd_metal: u16,
    cd_weapon: u16,
    cd_recalib: u16,
    frame_count: u32,
}

impl WeaponDetector {
    pub const fn new() -> Self {
        Self {
            baseline_amp_var: [0.0; MAX_SC],
            baseline_phase_var: [0.0; MAX_SC],
            cal_amp_sum: [0.0; MAX_SC],
            cal_amp_sq_sum: [0.0; MAX_SC],
            cal_phase_sum: [0.0; MAX_SC],
            cal_phase_sq_sum: [0.0; MAX_SC],
            cal_count: 0,
            calibrated: false,
            amp_window: [0.0; MAX_SC],
            phase_window: [0.0; MAX_SC],
            run_amp_mean: [0.0; MAX_SC],
            run_amp_m2: [0.0; MAX_SC],
            run_phase_mean: [0.0; MAX_SC],
            run_phase_m2: [0.0; MAX_SC],
            run_count: 0,
            metal_run: 0,
            weapon_run: 0,
            cd_metal: 0,
            cd_weapon: 0,
            cd_recalib: 0,
            frame_count: 0,
        }
    }

    /// Process one CSI frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        motion_energy: f32,
        presence: i32,
    ) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(variance.len()).min(MAX_SC);
        if n_sc < 2 {
            return &[];
        }

        self.frame_count += 1;
        self.cd_metal = self.cd_metal.saturating_sub(1);
        self.cd_weapon = self.cd_weapon.saturating_sub(1);
        self.cd_recalib = self.cd_recalib.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut ne = 0usize;

        // Calibration phase: collect baseline statistics in empty room.
        if !self.calibrated {
            for i in 0..n_sc {
                self.cal_amp_sum[i] += amplitudes[i];
                self.cal_amp_sq_sum[i] += amplitudes[i] * amplitudes[i];
                self.cal_phase_sum[i] += phases[i];
                self.cal_phase_sq_sum[i] += phases[i] * phases[i];
            }
            self.cal_count += 1;

            if self.cal_count >= BASELINE_FRAMES {
                let n = self.cal_count as f32;
                for i in 0..n_sc {
                    let amp_mean = self.cal_amp_sum[i] / n;
                    self.baseline_amp_var[i] =
                        (self.cal_amp_sq_sum[i] / n - amp_mean * amp_mean).max(0.001);
                    let phase_mean = self.cal_phase_sum[i] / n;
                    self.baseline_phase_var[i] =
                        (self.cal_phase_sq_sum[i] / n - phase_mean * phase_mean).max(0.001);
                }
                self.calibrated = true;
            }
            return unsafe { &EVENTS[..0] };
        }

        // Update running Welford statistics.
        self.run_count += 1;
        let rc = self.run_count as f32;
        for i in 0..n_sc {
            // Amplitude Welford.
            let delta_a = amplitudes[i] - self.run_amp_mean[i];
            self.run_amp_mean[i] += delta_a / rc;
            let delta2_a = amplitudes[i] - self.run_amp_mean[i];
            self.run_amp_m2[i] += delta_a * delta2_a;

            // Phase Welford.
            let delta_p = phases[i] - self.run_phase_mean[i];
            self.run_phase_mean[i] += delta_p / rc;
            let delta2_p = phases[i] - self.run_phase_mean[i];
            self.run_phase_m2[i] += delta_p * delta2_p;
        }

        // Only detect when someone is present and moving.
        if presence < MIN_PRESENCE || motion_energy < MIN_MOTION_ENERGY {
            self.metal_run = 0;
            self.weapon_run = 0;
            // Reset running stats periodically when no one is present.
            if self.run_count > 200 {
                self.run_count = 0;
                for i in 0..MAX_SC {
                    self.run_amp_mean[i] = 0.0;
                    self.run_amp_m2[i] = 0.0;
                    self.run_phase_mean[i] = 0.0;
                    self.run_phase_m2[i] = 0.0;
                }
            }
            return unsafe { &EVENTS[..0] };
        }

        // Compute current amplitude variance / phase variance ratio.
        if self.run_count < 4 {
            return unsafe { &EVENTS[..0] };
        }

        let mut ratio_sum = 0.0f32;
        let mut valid_sc = 0u32;
        let mut max_drift = 0.0f32;

        for i in 0..n_sc {
            let amp_var = self.run_amp_m2[i] / (self.run_count as f32 - 1.0);
            let phase_var = self.run_phase_m2[i] / (self.run_count as f32 - 1.0);

            if phase_var > 0.0001 {
                let ratio = amp_var / phase_var;
                ratio_sum += ratio;
                valid_sc += 1;
            }

            // Check for calibration drift.
            let drift = if self.baseline_amp_var[i] > 0.0001 {
                fabsf(amp_var - self.baseline_amp_var[i]) / self.baseline_amp_var[i]
            } else {
                0.0
            };
            if drift > max_drift {
                max_drift = drift;
            }
        }

        if valid_sc < 2 {
            return unsafe { &EVENTS[..0] };
        }

        let mean_ratio = ratio_sum / valid_sc as f32;

        // Check for re-calibration need.
        if max_drift > RECALIB_DRIFT_THRESH && self.cd_recalib == 0 && ne < 3 {
            unsafe { EVENTS[ne] = (EVENT_CALIBRATION_NEEDED, max_drift); }
            ne += 1;
            self.cd_recalib = COOLDOWN * 5; // Less frequent recalibration alerts.
        }

        // Metal anomaly detection.
        if mean_ratio > METAL_RATIO_THRESH {
            self.metal_run = self.metal_run.saturating_add(1);
        } else {
            self.metal_run = self.metal_run.saturating_sub(1);
        }

        // Weapon-grade detection (higher threshold).
        if mean_ratio > WEAPON_RATIO_THRESH {
            self.weapon_run = self.weapon_run.saturating_add(1);
        } else {
            self.weapon_run = self.weapon_run.saturating_sub(1);
        }

        // Emit metal anomaly.
        if self.metal_run >= METAL_DEBOUNCE && self.cd_metal == 0 && ne < 3 {
            unsafe { EVENTS[ne] = (EVENT_METAL_ANOMALY, mean_ratio); }
            ne += 1;
            self.cd_metal = COOLDOWN;
        }

        // Emit weapon alert (supersedes metal anomaly in severity).
        if self.weapon_run >= WEAPON_DEBOUNCE && self.cd_weapon == 0 && ne < 3 {
            unsafe { EVENTS[ne] = (EVENT_WEAPON_ALERT, mean_ratio); }
            ne += 1;
            self.cd_weapon = COOLDOWN;
        }

        unsafe { &EVENTS[..ne] }
    }

    pub fn is_calibrated(&self) -> bool { self.calibrated }
    pub fn frame_count(&self) -> u32 { self.frame_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let det = WeaponDetector::new();
        assert!(!det.is_calibrated());
        assert_eq!(det.frame_count(), 0);
    }

    #[test]
    fn test_calibration_completes() {
        let mut det = WeaponDetector::new();
        for i in 0..BASELINE_FRAMES {
            let p: [f32; 16] = {
                let mut arr = [0.0f32; 16];
                for j in 0..16 { arr[j] = (i as f32) * 0.01 + (j as f32) * 0.001; }
                arr
            };
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0, 0);
        }
        assert!(det.is_calibrated());
    }

    #[test]
    fn test_no_detection_without_presence() {
        let mut det = WeaponDetector::new();
        // Calibrate.
        for i in 0..BASELINE_FRAMES {
            let mut p = [0.0f32; 16];
            for j in 0..16 { p[j] = (i as f32) * 0.01; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0, 0);
        }

        // Send high-ratio data but with no presence.
        for i in 0..50u32 {
            let mut p = [0.0f32; 16];
            for j in 0..16 { p[j] = 5.0 + (i as f32) * 0.001; }
            // High amplitude, low phase change => high ratio, but presence = 0.
            let ev = det.process_frame(&p, &[20.0; 16], &[0.01; 16], 0.0, 0);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_METAL_ANOMALY);
                assert_ne!(et, EVENT_WEAPON_ALERT);
            }
        }
    }

    #[test]
    fn test_metal_anomaly_detection() {
        let mut det = WeaponDetector::new();
        // Calibrate with moderate signal (some variation for realistic baseline).
        for i in 0..BASELINE_FRAMES {
            let mut p = [0.0f32; 16];
            for j in 0..16 { p[j] = (i as f32) * 0.01 + (j as f32) * 0.001; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0, 0);
        }

        // Simulate person with metal: high amplitude variance, small but nonzero phase variance.
        // Metal = specular reflector => amplitude swings wildly between frames,
        // while phase changes only slightly (not zero, but much less than amplitude).
        let mut found_metal = false;
        for i in 0..60u32 {
            let mut p = [0.0f32; 16];
            // Phase changes slightly per frame (small variance, nonzero).
            for j in 0..16 { p[j] = 1.0 + (i as f32) * 0.02 + (j as f32) * 0.01; }
            // Amplitude varies hugely between frames (metal strong reflector).
            let mut a = [0.0f32; 16];
            for j in 0..16 {
                a[j] = if (i + j as u32) % 2 == 0 { 15.0 } else { 2.0 };
            }
            let ev = det.process_frame(&p, &a, &[0.01; 16], 2.0, 1);
            for &(et, _) in ev {
                if et == EVENT_METAL_ANOMALY {
                    found_metal = true;
                }
            }
        }
        assert!(found_metal, "metal anomaly should be detected");
    }

    #[test]
    fn test_normal_person_no_metal_alert() {
        let mut det = WeaponDetector::new();
        // Calibrate.
        for i in 0..BASELINE_FRAMES {
            let mut p = [0.0f32; 16];
            for j in 0..16 { p[j] = (i as f32) * 0.01; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0, 0);
        }

        // Normal person: both amplitude and phase vary proportionally.
        for i in 0..50u32 {
            let mut p = [0.0f32; 16];
            let mut a = [0.0f32; 16];
            for j in 0..16 {
                p[j] = 1.0 + (i as f32) * 0.1 + (j as f32) * 0.05;
                a[j] = 1.0 + (i as f32) * 0.1 + (j as f32) * 0.05;
            }
            let ev = det.process_frame(&p, &a, &[0.01; 16], 1.0, 1);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_WEAPON_ALERT, "normal person should not trigger weapon alert");
            }
        }
    }

    #[test]
    fn test_calibration_needed_on_drift() {
        let mut det = WeaponDetector::new();
        // Calibrate with low, stable amplitudes (small variance baseline).
        for i in 0..BASELINE_FRAMES {
            let mut p = [0.0f32; 16];
            let mut a = [0.0f32; 16];
            for j in 0..16 {
                p[j] = (i as f32) * 0.01;
                // Slight amplitude variation so baseline_amp_var is small but real.
                a[j] = 0.5 + (j as f32) * 0.01;
            }
            det.process_frame(&p, &a, &[0.01; 16], 0.0, 0);
        }

        // Drastically different environment: huge amplitude swings => large running
        // variance that differs vastly from the small calibration baseline.
        let mut found_recalib = false;
        for i in 0..60u32 {
            let mut p = [0.0f32; 16];
            let mut a = [0.0f32; 16];
            for j in 0..16 {
                p[j] = 10.0 + (i as f32) * 0.05;
                // Wildly varying amplitudes per frame to build large running variance.
                a[j] = if i % 2 == 0 { 50.0 } else { 5.0 };
            }
            let ev = det.process_frame(&p, &a, &[10.0; 16], 3.0, 1);
            for &(et, _) in ev {
                if et == EVENT_CALIBRATION_NEEDED {
                    found_recalib = true;
                }
            }
        }
        assert!(found_recalib, "calibration needed should trigger on large drift");
    }

    #[test]
    fn test_too_few_subcarriers() {
        let mut det = WeaponDetector::new();
        let ev = det.process_frame(&[0.1], &[1.0], &[0.01], 0.0, 0);
        assert!(ev.is_empty(), "should return empty with < 2 subcarriers");
    }
}

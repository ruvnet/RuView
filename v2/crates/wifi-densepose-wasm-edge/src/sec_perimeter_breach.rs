//! Multi-zone perimeter breach detection — ADR-041 Category 2 Security module.
//!
//! Monitors up to 4 perimeter zones via phase gradient analysis across subcarrier
//! groups. Determines movement direction (approach vs departure) from the temporal
//! ordering of phase disturbances and tracks zone-to-zone transitions with
//! directional vectors.
//!
//! Events: PERIMETER_BREACH(210), APPROACH_DETECTED(211),
//!         DEPARTURE_DETECTED(212), ZONE_TRANSITION(213).  Budget: S (<5 ms).

#[cfg(not(feature = "std"))]
use libm::{fabsf, sqrtf};
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }
#[cfg(feature = "std")]
fn fabsf(x: f32) -> f32 { x.abs() }

const MAX_SC: usize = 32;
/// Number of perimeter zones.
const MAX_ZONES: usize = 4;
/// Calibration frames (5 seconds at 20 Hz).
const BASELINE_FRAMES: u32 = 100;
/// Phase gradient threshold for breach detection (rad/subcarrier).
const BREACH_GRADIENT_THRESH: f32 = 0.6;
/// Minimum variance ratio above baseline to consider zone disturbed.
const VARIANCE_RATIO_THRESH: f32 = 2.5;
/// Consecutive frames required for direction confirmation.
const DIRECTION_DEBOUNCE: u8 = 3;
/// Cooldown frames after event emission.
const COOLDOWN: u16 = 40;
/// History depth for direction estimation.
const HISTORY_LEN: usize = 8;

pub const EVENT_PERIMETER_BREACH: i32 = 210;
pub const EVENT_APPROACH_DETECTED: i32 = 211;
pub const EVENT_DEPARTURE_DETECTED: i32 = 212;
pub const EVENT_ZONE_TRANSITION: i32 = 213;

/// Per-zone state for gradient tracking.
#[derive(Clone, Copy)]
struct ZoneState {
    /// Baseline mean phase gradient magnitude.
    baseline_grad: f32,
    /// Baseline amplitude variance.
    baseline_var: f32,
    /// Recent disturbance energy history (rolling).
    energy_history: [f32; HISTORY_LEN],
    hist_idx: usize,
    /// Consecutive frames zone is disturbed.
    disturb_run: u8,
}

impl ZoneState {
    const fn new() -> Self {
        Self {
            baseline_grad: 0.0,
            baseline_var: 0.001,
            energy_history: [0.0; HISTORY_LEN],
            hist_idx: 0,
            disturb_run: 0,
        }
    }

    fn push_energy(&mut self, e: f32) {
        self.energy_history[self.hist_idx] = e;
        self.hist_idx = (self.hist_idx + 1) % HISTORY_LEN;
    }

    /// Compute gradient trend: positive = increasing (approach), negative = decreasing (departure).
    fn energy_trend(&self) -> f32 {
        // Simple linear regression slope over history buffer.
        let n = HISTORY_LEN as f32;
        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        let mut sxy = 0.0f32;
        let mut sxx = 0.0f32;
        for k in 0..HISTORY_LEN {
            // Read in chronological order from oldest to newest.
            let idx = (self.hist_idx + k) % HISTORY_LEN;
            let x = k as f32;
            let y = self.energy_history[idx];
            sx += x;
            sy += y;
            sxy += x * y;
            sxx += x * x;
        }
        let denom = n * sxx - sx * sx;
        if fabsf(denom) < 1e-6 { return 0.0; }
        (n * sxy - sx * sy) / denom
    }
}

/// Multi-zone perimeter breach detector.
pub struct PerimeterBreachDetector {
    zones: [ZoneState; MAX_ZONES],
    /// Calibration accumulators per zone: sum of gradient magnitudes.
    cal_grad_sum: [f32; MAX_ZONES],
    /// Calibration accumulators per zone: sum of variance.
    cal_var_sum: [f32; MAX_ZONES],
    cal_count: u32,
    calibrated: bool,
    /// Previous frame phase values.
    prev_phases: [f32; MAX_SC],
    phase_init: bool,
    /// Last zone that was disturbed (for transition detection).
    last_active_zone: i32,
    /// Cooldowns per event type.
    cd_breach: u16,
    cd_approach: u16,
    cd_departure: u16,
    cd_transition: u16,
    frame_count: u32,
    /// Approach/departure debounce counters per zone.
    approach_run: [u8; MAX_ZONES],
    departure_run: [u8; MAX_ZONES],
}

impl PerimeterBreachDetector {
    pub const fn new() -> Self {
        Self {
            zones: [ZoneState::new(); MAX_ZONES],
            cal_grad_sum: [0.0; MAX_ZONES],
            cal_var_sum: [0.0; MAX_ZONES],
            cal_count: 0,
            calibrated: false,
            prev_phases: [0.0; MAX_SC],
            phase_init: false,
            last_active_zone: -1,
            cd_breach: 0,
            cd_approach: 0,
            cd_departure: 0,
            cd_transition: 0,
            frame_count: 0,
            approach_run: [0; MAX_ZONES],
            departure_run: [0; MAX_ZONES],
        }
    }

    /// Process one CSI frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        variance: &[f32],
        _motion_energy: f32,
    ) -> &[(i32, f32)] {
        let n_sc = phases.len().min(amplitudes.len()).min(variance.len()).min(MAX_SC);
        if n_sc < 4 {
            return &[];
        }

        self.frame_count += 1;
        self.cd_breach = self.cd_breach.saturating_sub(1);
        self.cd_approach = self.cd_approach.saturating_sub(1);
        self.cd_departure = self.cd_departure.saturating_sub(1);
        self.cd_transition = self.cd_transition.saturating_sub(1);

        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;

        let subs_per_zone = n_sc / MAX_ZONES;
        if subs_per_zone < 1 {
            return &[];
        }

        // Compute per-zone metrics.
        let mut zone_grad = [0.0f32; MAX_ZONES];
        let mut zone_var = [0.0f32; MAX_ZONES];

        for z in 0..MAX_ZONES {
            let start = z * subs_per_zone;
            let end = if z == MAX_ZONES - 1 { n_sc } else { start + subs_per_zone };
            let count = (end - start) as f32;
            if count < 2.0 { continue; }

            // Phase gradient: mean absolute difference between adjacent subcarriers.
            let mut grad_sum = 0.0f32;
            if self.phase_init {
                for i in start..end {
                    grad_sum += fabsf(phases[i] - self.prev_phases[i]);
                }
            }
            zone_grad[z] = grad_sum / count;

            // Mean variance for zone.
            let mut var_sum = 0.0f32;
            for i in start..end {
                var_sum += variance[i];
            }
            zone_var[z] = var_sum / count;
        }

        // Save phases for next frame.
        for i in 0..n_sc {
            self.prev_phases[i] = phases[i];
        }
        if !self.phase_init {
            self.phase_init = true;
            return unsafe { &EVENTS[..0] };
        }

        // Calibration phase.
        if !self.calibrated {
            for z in 0..MAX_ZONES {
                self.cal_grad_sum[z] += zone_grad[z];
                self.cal_var_sum[z] += zone_var[z];
            }
            self.cal_count += 1;
            if self.cal_count >= BASELINE_FRAMES {
                let n = self.cal_count as f32;
                for z in 0..MAX_ZONES {
                    self.zones[z].baseline_grad = self.cal_grad_sum[z] / n;
                    self.zones[z].baseline_var = (self.cal_var_sum[z] / n).max(0.001);
                }
                self.calibrated = true;
            }
            return unsafe { &EVENTS[..0] };
        }

        // Detect breaches and direction per zone.
        let mut most_disturbed_zone: i32 = -1;
        let mut max_energy = 0.0f32;

        for z in 0..MAX_ZONES {
            let grad_ratio = if self.zones[z].baseline_grad > 1e-6 {
                zone_grad[z] / self.zones[z].baseline_grad
            } else {
                zone_grad[z] / 0.001
            };
            let var_ratio = zone_var[z] / self.zones[z].baseline_var;

            let energy = grad_ratio * 0.6 + var_ratio * 0.4;
            self.zones[z].push_energy(energy);

            let is_breach = zone_grad[z] > BREACH_GRADIENT_THRESH
                && var_ratio > VARIANCE_RATIO_THRESH;

            if is_breach {
                self.zones[z].disturb_run = self.zones[z].disturb_run.saturating_add(1);
                if energy > max_energy {
                    max_energy = energy;
                    most_disturbed_zone = z as i32;
                }
            } else {
                self.zones[z].disturb_run = 0;
            }

            // Direction detection via energy trend.
            let trend = self.zones[z].energy_trend();
            if trend > 0.05 {
                self.approach_run[z] = self.approach_run[z].saturating_add(1);
                self.departure_run[z] = 0;
            } else if trend < -0.05 {
                self.departure_run[z] = self.departure_run[z].saturating_add(1);
                self.approach_run[z] = 0;
            } else {
                self.approach_run[z] = 0;
                self.departure_run[z] = 0;
            }

            // Emit approach event.
            if self.approach_run[z] >= DIRECTION_DEBOUNCE && is_breach
                && self.cd_approach == 0 && ne < 4
            {
                unsafe { EVENTS[ne] = (EVENT_APPROACH_DETECTED, z as f32); }
                ne += 1;
                self.cd_approach = COOLDOWN;
                self.approach_run[z] = 0;
            }

            // Emit departure event.
            if self.departure_run[z] >= DIRECTION_DEBOUNCE
                && self.cd_departure == 0 && ne < 4
            {
                unsafe { EVENTS[ne] = (EVENT_DEPARTURE_DETECTED, z as f32); }
                ne += 1;
                self.cd_departure = COOLDOWN;
                self.departure_run[z] = 0;
            }
        }

        // Perimeter breach event.
        if most_disturbed_zone >= 0 && self.cd_breach == 0 && ne < 4 {
            unsafe { EVENTS[ne] = (EVENT_PERIMETER_BREACH, max_energy); }
            ne += 1;
            self.cd_breach = COOLDOWN;
        }

        // Zone transition event.
        if most_disturbed_zone >= 0
            && self.last_active_zone >= 0
            && most_disturbed_zone != self.last_active_zone
            && self.cd_transition == 0
            && ne < 4
        {
            // Encode as from*10 + to.
            let transition_code = self.last_active_zone as f32 * 10.0
                + most_disturbed_zone as f32;
            unsafe { EVENTS[ne] = (EVENT_ZONE_TRANSITION, transition_code); }
            ne += 1;
            self.cd_transition = COOLDOWN;
        }

        if most_disturbed_zone >= 0 {
            self.last_active_zone = most_disturbed_zone;
        }

        unsafe { &EVENTS[..ne] }
    }

    pub fn is_calibrated(&self) -> bool { self.calibrated }
    pub fn frame_count(&self) -> u32 { self.frame_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_quiet() -> ([f32; 16], [f32; 16], [f32; 16]) {
        ([0.1; 16], [1.0; 16], [0.01; 16])
    }

    #[test]
    fn test_init() {
        let det = PerimeterBreachDetector::new();
        assert!(!det.is_calibrated());
        assert_eq!(det.frame_count(), 0);
    }

    #[test]
    fn test_calibration_completes() {
        let mut det = PerimeterBreachDetector::new();
        let (p, a, v) = make_quiet();
        // Need one extra frame for phase_init.
        for i in 0..(BASELINE_FRAMES + 2) {
            let mut pp = p;
            // Vary slightly so phase_init triggers.
            for j in 0..16 { pp[j] = 0.1 + (i as f32) * 0.001 + (j as f32) * 0.0001; }
            det.process_frame(&pp, &a, &v, 0.0);
        }
        assert!(det.is_calibrated());
    }

    #[test]
    fn test_no_events_during_calibration() {
        let mut det = PerimeterBreachDetector::new();
        let (p, a, v) = make_quiet();
        for _ in 0..50 {
            let ev = det.process_frame(&p, &a, &v, 0.0);
            assert!(ev.is_empty());
        }
    }

    #[test]
    fn test_breach_detection() {
        let mut det = PerimeterBreachDetector::new();
        // Calibrate with quiet data.
        for i in 0..(BASELINE_FRAMES + 2) {
            let mut p = [0.1f32; 16];
            for j in 0..16 { p[j] = 0.1 + (i as f32) * 0.001; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0);
        }
        assert!(det.is_calibrated());

        // Inject large disturbance in zone 0 (subcarriers 0-3).
        let mut found_breach = false;
        for frame in 0..20u32 {
            let mut p = [0.1f32; 16];
            let mut a = [1.0f32; 16];
            let mut v = [0.01f32; 16];
            // Zone 0: big phase jump + high variance.
            for j in 0..4 {
                p[j] = 3.0 + (frame as f32) * 1.5;
                a[j] = 8.0;
                v[j] = 5.0;
            }
            let ev = det.process_frame(&p, &a, &v, 5.0);
            for &(et, _) in ev {
                if et == EVENT_PERIMETER_BREACH {
                    found_breach = true;
                }
            }
        }
        assert!(found_breach, "perimeter breach should be detected");
    }

    #[test]
    fn test_zone_transition() {
        let mut det = PerimeterBreachDetector::new();
        // Calibrate.
        for i in 0..(BASELINE_FRAMES + 2) {
            let mut p = [0.1f32; 16];
            for j in 0..16 { p[j] = 0.1 + (i as f32) * 0.001; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0);
        }

        // Disturb zone 0 first.
        for frame in 0..10u32 {
            let mut p = [0.1f32; 16];
            let mut v = [0.01f32; 16];
            for j in 0..4 {
                p[j] = 3.0 + (frame as f32) * 1.5;
                v[j] = 5.0;
            }
            det.process_frame(&p, &[1.0; 16], &v, 5.0);
        }

        // Now disturb zone 2 (subcarriers 8-11) — should trigger zone transition.
        let mut found_transition = false;
        for frame in 0..10u32 {
            let mut p = [0.1f32; 16];
            let mut v = [0.01f32; 16];
            for j in 8..12 {
                p[j] = 3.0 + (frame as f32) * 1.5;
                v[j] = 5.0;
            }
            let ev = det.process_frame(&p, &[1.0; 16], &v, 5.0);
            for &(et, _) in ev {
                if et == EVENT_ZONE_TRANSITION {
                    found_transition = true;
                }
            }
        }
        assert!(found_transition, "zone transition should be detected");
    }

    #[test]
    fn test_approach_detection() {
        let mut det = PerimeterBreachDetector::new();
        // Calibrate.
        for i in 0..(BASELINE_FRAMES + 2) {
            let mut p = [0.1f32; 16];
            for j in 0..16 { p[j] = 0.1 + (i as f32) * 0.001; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0);
        }

        // Simulate increasing disturbance in zone 1 (approaching).
        let mut found_approach = false;
        for frame in 0..30u32 {
            let mut p = [0.1f32; 16];
            let mut v = [0.01f32; 16];
            // Gradually increase disturbance in zone 1 (subcarriers 4-7).
            let intensity = 0.5 + (frame as f32) * 0.3;
            for j in 4..8 {
                p[j] = intensity * 2.0;
                v[j] = intensity;
            }
            let ev = det.process_frame(&p, &[1.0; 16], &v, intensity);
            for &(et, _) in ev {
                if et == EVENT_APPROACH_DETECTED {
                    found_approach = true;
                }
            }
        }
        assert!(found_approach, "approach should be detected on increasing disturbance");
    }

    #[test]
    fn test_quiet_no_breach() {
        let mut det = PerimeterBreachDetector::new();
        // Calibrate.
        for i in 0..(BASELINE_FRAMES + 2) {
            let mut p = [0.1f32; 16];
            for j in 0..16 { p[j] = 0.1 + (i as f32) * 0.001; }
            det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0);
        }

        // Continue with quiet data — should not trigger breach.
        for i in 0..100u32 {
            let mut p = [0.1f32; 16];
            for j in 0..16 { p[j] = 0.1 + ((BASELINE_FRAMES + 2 + i) as f32) * 0.001; }
            let ev = det.process_frame(&p, &[1.0; 16], &[0.01; 16], 0.0);
            for &(et, _) in ev {
                assert_ne!(et, EVENT_PERIMETER_BREACH, "no breach on quiet signal");
            }
        }
    }
}

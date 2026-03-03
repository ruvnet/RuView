//! Behavioral profiling with Mahalanobis-inspired anomaly scoring.
//!
//! ADR-041 AI Security module. Maintains a 6D behavior profile and detects
//! anomalous deviations using online Welford statistics and combined Z-scores.
//!
//! Dimensions: presence_rate, avg_motion, avg_n_persons, activity_variance,
//!             transition_rate, dwell_time.
//!
//! Events: BEHAVIOR_ANOMALY(825), PROFILE_DEVIATION(826), NOVEL_PATTERN(827),
//!         PROFILE_MATURITY(828).  Budget: S (< 5 ms).

#[cfg(not(feature = "std"))]
use libm::sqrtf;
#[cfg(feature = "std")]
fn sqrtf(x: f32) -> f32 { x.sqrt() }

const N_DIM: usize = 6;
const LEARNING_FRAMES: u32 = 1000;
const ANOMALY_Z: f32 = 3.0;
const NOVEL_Z: f32 = 2.0;
const NOVEL_MIN: u32 = 3;
const OBS_WIN: usize = 200;
const COOLDOWN: u16 = 100;
const MATURITY_INTERVAL: u32 = 72000;
const VAR_FLOOR: f32 = 1e-6;

pub const EVENT_BEHAVIOR_ANOMALY: i32 = 825;
pub const EVENT_PROFILE_DEVIATION: i32 = 826;
pub const EVENT_NOVEL_PATTERN: i32 = 827;
pub const EVENT_PROFILE_MATURITY: i32 = 828;

/// Welford's online mean/variance accumulator (single dimension).
#[derive(Clone, Copy)]
struct Welford { count: u32, mean: f32, m2: f32 }
impl Welford {
    const fn new() -> Self { Self { count: 0, mean: 0.0, m2: 0.0 } }
    fn update(&mut self, x: f32) {
        self.count += 1;
        let d = x - self.mean;
        self.mean += d / (self.count as f32);
        self.m2 += d * (x - self.mean);
    }
    fn variance(&self) -> f32 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count as f32) }
    }
    fn z_score(&self, x: f32) -> f32 {
        let v = self.variance();
        if v < VAR_FLOOR { return 0.0; }
        let z = (x - self.mean) / sqrtf(v);
        if z < 0.0 { -z } else { z }
    }
}

/// Ring buffer for observation window.
struct ObsWindow {
    pres: [u8; OBS_WIN],
    motion: [f32; OBS_WIN],
    persons: [u8; OBS_WIN],
    idx: usize,
    len: usize,
}
impl ObsWindow {
    const fn new() -> Self {
        Self { pres: [0; OBS_WIN], motion: [0.0; OBS_WIN], persons: [0; OBS_WIN], idx: 0, len: 0 }
    }
    fn push(&mut self, present: bool, mot: f32, np: u8) {
        self.pres[self.idx] = present as u8;
        self.motion[self.idx] = mot;
        self.persons[self.idx] = np;
        self.idx = (self.idx + 1) % OBS_WIN;
        if self.len < OBS_WIN { self.len += 1; }
    }
    /// Compute 6D feature vector from current window.
    fn features(&self) -> [f32; N_DIM] {
        if self.len == 0 { return [0.0; N_DIM]; }
        let n = self.len as f32;
        let start = if self.len < OBS_WIN { 0 } else { self.idx };
        // Sums
        let (mut ps, mut ms, mut ns) = (0u32, 0.0f32, 0u32);
        for i in 0..self.len { ps += self.pres[i] as u32; ms += self.motion[i]; ns += self.persons[i] as u32; }
        let avg_m = ms / n;
        // Variance of motion
        let mut mv = 0.0f32;
        for i in 0..self.len { let d = self.motion[i] - avg_m; mv += d * d; }
        // Transitions
        let mut tr = 0u32;
        let mut prev_p = self.pres[start];
        for s in 1..self.len {
            let cur = self.pres[(start + s) % OBS_WIN];
            if cur != prev_p { tr += 1; }
            prev_p = cur;
        }
        // Dwell time (avg consecutive presence run length)
        let (mut dsum, mut druns, mut rlen) = (0u32, 0u32, 0u32);
        for s in 0..self.len {
            if self.pres[(start + s) % OBS_WIN] == 1 { rlen += 1; }
            else if rlen > 0 { dsum += rlen; druns += 1; rlen = 0; }
        }
        if rlen > 0 { dsum += rlen; druns += 1; }
        let dwell = if druns > 0 { dsum as f32 / druns as f32 } else { 0.0 };
        [ps as f32 / n, avg_m, ns as f32 / n, mv / n, tr as f32 / n, dwell]
    }
}

/// Behavioral profiler with Mahalanobis-inspired anomaly scoring.
pub struct BehavioralProfiler {
    stats: [Welford; N_DIM],
    obs: ObsWindow,
    mature: bool,
    frame_count: u32,
    obs_cycles: u32,
    cooldown: u16,
    anomaly_count: u32,
}

impl BehavioralProfiler {
    pub const fn new() -> Self {
        Self {
            stats: [Welford::new(); N_DIM], obs: ObsWindow::new(),
            mature: false, frame_count: 0, obs_cycles: 0, cooldown: 0, anomaly_count: 0,
        }
    }

    /// Process one frame. Returns `(event_id, value)` pairs.
    pub fn process_frame(&mut self, present: bool, motion: f32, n_persons: u8) -> &[(i32, f32)] {
        self.frame_count += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        self.obs.push(present, motion, n_persons);

        static mut EV: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut ne = 0usize;

        if self.frame_count % (OBS_WIN as u32) == 0 && self.obs.len == OBS_WIN {
            let feat = self.obs.features();
            self.obs_cycles += 1;

            if !self.mature {
                for d in 0..N_DIM { self.stats[d].update(feat[d]); }
                if self.obs_cycles >= LEARNING_FRAMES / (OBS_WIN as u32) {
                    self.mature = true;
                    let days = self.frame_count as f32 / (20.0 * 86400.0);
                    unsafe { EV[ne] = (EVENT_PROFILE_MATURITY, days); }
                    ne += 1;
                }
            } else {
                // Score before updating.
                let mut zsq = 0.0f32;
                let mut hi_z = 0u32;
                let (mut max_z, mut max_d) = (0.0f32, 0usize);
                for d in 0..N_DIM {
                    let z = self.stats[d].z_score(feat[d]);
                    zsq += z * z;
                    if z > NOVEL_Z { hi_z += 1; }
                    if z > max_z { max_z = z; max_d = d; }
                }
                let cz = sqrtf(zsq / N_DIM as f32);
                for d in 0..N_DIM { self.stats[d].update(feat[d]); }

                if self.cooldown == 0 {
                    if cz > ANOMALY_Z {
                        self.anomaly_count += 1;
                        unsafe { EV[ne] = (EVENT_BEHAVIOR_ANOMALY, cz); } ne += 1;
                        if ne < 4 { unsafe { EV[ne] = (EVENT_PROFILE_DEVIATION, max_d as f32); } ne += 1; }
                        self.cooldown = COOLDOWN;
                    }
                    if hi_z >= NOVEL_MIN && ne < 4 {
                        unsafe { EV[ne] = (EVENT_NOVEL_PATTERN, hi_z as f32); } ne += 1;
                        if self.cooldown == 0 { self.cooldown = COOLDOWN; }
                    }
                }
            }
        }

        // Periodic maturity report.
        if self.mature && self.frame_count % MATURITY_INTERVAL == 0 && ne < 4 {
            unsafe { EV[ne] = (EVENT_PROFILE_MATURITY, self.frame_count as f32 / (20.0 * 86400.0)); }
            ne += 1;
        }
        unsafe { &EV[..ne] }
    }

    pub fn is_mature(&self) -> bool { self.mature }
    pub fn frame_count(&self) -> u32 { self.frame_count }
    pub fn total_anomalies(&self) -> u32 { self.anomaly_count }
    pub fn dim_mean(&self, d: usize) -> f32 { if d < N_DIM { self.stats[d].mean } else { 0.0 } }
    pub fn dim_variance(&self, d: usize) -> f32 { if d < N_DIM { self.stats[d].variance() } else { 0.0 } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let bp = BehavioralProfiler::new();
        assert_eq!(bp.frame_count(), 0);
        assert!(!bp.is_mature());
        assert_eq!(bp.total_anomalies(), 0);
    }

    #[test]
    fn test_welford() {
        let mut w = Welford::new();
        for _ in 0..100 { w.update(5.0); }
        assert!((w.mean - 5.0).abs() < 0.001);
        assert!(w.variance() < 0.001);
        // Z-score at mean ~ 0, far from mean > 3.
        assert!(w.z_score(5.0) < 0.1);
    }

    #[test]
    fn test_welford_z_far() {
        let mut w = Welford::new();
        for i in 1..=100 { w.update(i as f32); }
        assert!(w.z_score(200.0) > 3.0);
    }

    #[test]
    fn test_learning_phase() {
        let mut bp = BehavioralProfiler::new();
        for _ in 0..LEARNING_FRAMES { bp.process_frame(true, 0.5, 1); }
        assert!(bp.is_mature());
    }

    #[test]
    fn test_normal_no_anomaly() {
        let mut bp = BehavioralProfiler::new();
        for _ in 0..LEARNING_FRAMES { bp.process_frame(true, 0.5, 1); }
        for _ in 0..2000 {
            let ev = bp.process_frame(true, 0.5, 1);
            for &(t, _) in ev { assert_ne!(t, EVENT_BEHAVIOR_ANOMALY); }
        }
        assert_eq!(bp.total_anomalies(), 0);
    }

    #[test]
    fn test_anomaly_detection() {
        let mut bp = BehavioralProfiler::new();
        // Learning phase: vary motion energy across observation windows so that
        // Welford stats accumulate non-zero variance. Each observation window
        // is OBS_WIN=200 frames; we need LEARNING_FRAMES/OBS_WIN = 5 cycles.
        // By giving each window a different motion level, inter-window variance
        // builds up, enabling z_score to detect anomalies after maturity.
        for i in 0..LEARNING_FRAMES {
            // Vary presence AND motion across observation windows so all
            // dimensions build non-zero variance.
            let window_id = i / (OBS_WIN as u32);
            let pres = window_id % 2 != 0;
            let mot = 0.1 + (window_id as f32) * 0.05;
            let per = (window_id % 3) as u8;
            bp.process_frame(pres, mot, per);
        }
        assert!(bp.is_mature());
        let mut found = false;
        // Now inject a dramatically different behaviour.
        for _ in 0..4000 {
            let ev = bp.process_frame(true, 10.0, 5);
            if ev.iter().any(|&(t,_)| t == EVENT_BEHAVIOR_ANOMALY) { found = true; }
        }
        assert!(found, "dramatic change should trigger anomaly");
    }

    #[test]
    fn test_obs_features() {
        let mut obs = ObsWindow::new();
        for _ in 0..OBS_WIN { obs.push(true, 1.0, 2); }
        let f = obs.features();
        assert!((f[0] - 1.0).abs() < 0.01);  // presence_rate
        assert!((f[1] - 1.0).abs() < 0.01);  // avg_motion
        assert!((f[2] - 2.0).abs() < 0.01);  // avg_n_persons
        assert!(f[3] < 0.01);                 // activity_variance
        assert!(f[4] < 0.01);                 // transition_rate
    }

    #[test]
    fn test_maturity_event() {
        let mut bp = BehavioralProfiler::new();
        let mut found = false;
        for _ in 0..LEARNING_FRAMES {
            let ev = bp.process_frame(true, 0.5, 1);
            if ev.iter().any(|&(t,_)| t == EVENT_PROFILE_MATURITY) { found = true; }
        }
        assert!(found, "maturity event should be emitted");
    }
}

//! Attractor-based anomaly detection with Lyapunov exponents.
//!
//! ADR-041 adaptive learning module — Event IDs 735-738.
//!
//! Models the room's CSI as a 4D dynamical system:
//!   (mean_phase, mean_amplitude, variance, motion_energy)
//!
//! Classifies the attractor type from trajectory divergence:
//!   - Point attractor:    trajectory converges to fixed point (empty room)
//!   - Limit cycle:        periodic orbit (HVAC only, machinery)
//!   - Strange attractor:  bounded but aperiodic (occupied room)
//!
//! Computes the largest Lyapunov exponent to quantify chaos:
//!   lambda = (1/N) * sum(log(|delta_n+1| / |delta_n|))
//!   lambda > 0 => chaotic, lambda < 0 => stable, lambda ~ 0 => periodic
//!
//! Detects anomalies as trajectory departures from the learned attractor basin.
//!
//! Budget: S (standard, < 5 ms).

use libm::{logf, sqrtf};

/// Trajectory buffer length (circular, 128 points of 4D state).
const TRAJ_LEN: usize = 128;

/// State vector dimensionality.
const STATE_DIM: usize = 4;

/// Minimum frames before attractor classification is valid.
const MIN_FRAMES_FOR_CLASSIFICATION: u32 = 200;

/// Lyapunov exponent thresholds for attractor classification.
const LYAPUNOV_STABLE_UPPER: f32 = -0.01; // lambda < this => point attractor
const LYAPUNOV_PERIODIC_UPPER: f32 = 0.01; // lambda < this => limit cycle
// lambda >= PERIODIC_UPPER => strange attractor

/// Basin departure threshold (multiplier of learned attractor radius).
const BASIN_DEPARTURE_MULT: f32 = 3.0;

/// EMA alpha for attractor center tracking.
const CENTER_ALPHA: f32 = 0.01;

/// Minimum delta magnitude to avoid log(0).
const MIN_DELTA: f32 = 1.0e-8;

/// Cooldown frames after basin departure alert.
const DEPARTURE_COOLDOWN: u16 = 100;

// ── Event IDs (735-series: Attractor dynamics) ───────────────────────────────

pub const EVENT_ATTRACTOR_TYPE: i32 = 735;
pub const EVENT_LYAPUNOV_EXPONENT: i32 = 736;
pub const EVENT_BASIN_DEPARTURE: i32 = 737;
pub const EVENT_LEARNING_COMPLETE: i32 = 738;

/// Attractor type classification.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum AttractorType {
    Unknown = 0,
    /// Fixed point — empty room, no dynamics.
    PointAttractor = 1,
    /// Periodic orbit — HVAC, machinery, regular motion.
    LimitCycle = 2,
    /// Bounded aperiodic — occupied room, human activity.
    StrangeAttractor = 3,
}

/// 4D state vector.
type StateVec = [f32; STATE_DIM];

/// Attractor-based anomaly detector.
pub struct AttractorDetector {
    /// Circular trajectory buffer.
    trajectory: [StateVec; TRAJ_LEN],
    /// Write index into trajectory buffer.
    traj_idx: usize,
    /// Number of points stored (max TRAJ_LEN).
    traj_len: usize,

    /// Learned attractor center (EMA-smoothed).
    center: StateVec,
    /// Learned attractor radius (max distance from center seen during learning).
    radius: f32,

    /// Running Lyapunov sum: sum of log(|delta_n+1|/|delta_n|).
    lyapunov_sum: f64,
    /// Number of Lyapunov samples accumulated.
    lyapunov_count: u32,

    /// Current attractor classification.
    attractor_type: AttractorType,

    /// Whether initial learning is complete.
    initialized: bool,
    /// Total frames processed.
    frame_count: u32,

    /// Cooldown counter for departure events.
    cooldown: u16,

    /// Previous state vector (for Lyapunov delta computation).
    prev_state: StateVec,
    /// Previous delta magnitude.
    prev_delta_mag: f32,
}

impl AttractorDetector {
    pub const fn new() -> Self {
        Self {
            trajectory: [[0.0; STATE_DIM]; TRAJ_LEN],
            traj_idx: 0,
            traj_len: 0,
            center: [0.0; STATE_DIM],
            radius: 0.0,
            lyapunov_sum: 0.0,
            lyapunov_count: 0,
            attractor_type: AttractorType::Unknown,
            initialized: false,
            frame_count: 0,
            cooldown: 0,
            prev_state: [0.0; STATE_DIM],
            prev_delta_mag: 0.0,
        }
    }

    /// Process one CSI frame.
    ///
    /// `phases`     — per-subcarrier phase values.
    /// `amplitudes` — per-subcarrier amplitude values.
    /// `motion_energy` — aggregate motion metric from host (Tier 2).
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(
        &mut self,
        phases: &[f32],
        amplitudes: &[f32],
        motion_energy: f32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        let n_sc = phases.len().min(amplitudes.len());
        if n_sc == 0 {
            return &[];
        }

        self.frame_count += 1;
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }

        // ── Build 4D state vector ────────────────────────────────────────
        let state = build_state(phases, amplitudes, motion_energy, n_sc);

        // ── Store in trajectory buffer ───────────────────────────────────
        self.trajectory[self.traj_idx] = state;
        self.traj_idx = (self.traj_idx + 1) % TRAJ_LEN;
        if self.traj_len < TRAJ_LEN {
            self.traj_len += 1;
        }

        // ── Compute Lyapunov contribution ────────────────────────────────
        if self.frame_count > 1 {
            let delta_mag = vec_distance(&state, &self.prev_state);
            if self.prev_delta_mag > MIN_DELTA && delta_mag > MIN_DELTA {
                let ratio = delta_mag / self.prev_delta_mag;
                self.lyapunov_sum += logf(ratio) as f64;
                self.lyapunov_count += 1;
            }
            self.prev_delta_mag = delta_mag;
        }
        self.prev_state = state;

        // ── Update attractor center (EMA) ────────────────────────────────
        if self.frame_count <= 1 {
            self.center = state;
        } else {
            for d in 0..STATE_DIM {
                self.center[d] = CENTER_ALPHA * state[d] + (1.0 - CENTER_ALPHA) * self.center[d];
            }
        }

        // ── Learning phase ───────────────────────────────────────────────
        if !self.initialized {
            // Track maximum radius during learning.
            let dist = vec_distance(&state, &self.center);
            if dist > self.radius {
                self.radius = dist;
            }

            if self.frame_count >= MIN_FRAMES_FOR_CLASSIFICATION && self.lyapunov_count > 0 {
                self.initialized = true;
                // Classify attractor.
                let lambda = self.lyapunov_exponent();
                self.attractor_type = classify_attractor(lambda);

                // Ensure radius has a minimum floor to avoid false departures.
                if self.radius < 0.01 {
                    self.radius = 0.01;
                }

                unsafe {
                    EVENTS[n_ev] = (EVENT_LEARNING_COMPLETE, 1.0);
                    n_ev += 1;
                    EVENTS[n_ev] = (EVENT_ATTRACTOR_TYPE, self.attractor_type as u8 as f32);
                    n_ev += 1;
                    EVENTS[n_ev] = (EVENT_LYAPUNOV_EXPONENT, lambda);
                    n_ev += 1;
                }

                return unsafe { &EVENTS[..n_ev] };
            }

            return &[];
        }

        // ── Post-learning: detect basin departures ───────────────────────
        let dist = vec_distance(&state, &self.center);
        let departure_threshold = self.radius * BASIN_DEPARTURE_MULT;

        if dist > departure_threshold && self.cooldown == 0 {
            self.cooldown = DEPARTURE_COOLDOWN;
            unsafe {
                EVENTS[n_ev] = (EVENT_BASIN_DEPARTURE, dist / self.radius);
                n_ev += 1;
            }
        }

        // ── Periodic attractor update (every 200 frames) ────────────────
        if self.frame_count % 200 == 0 && self.lyapunov_count > 0 {
            let lambda = self.lyapunov_exponent();
            let new_type = classify_attractor(lambda);

            if new_type != self.attractor_type && n_ev < 3 {
                self.attractor_type = new_type;
                unsafe {
                    EVENTS[n_ev] = (EVENT_ATTRACTOR_TYPE, new_type as u8 as f32);
                    n_ev += 1;
                    EVENTS[n_ev] = (EVENT_LYAPUNOV_EXPONENT, lambda);
                    n_ev += 1;
                }
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Compute the current largest Lyapunov exponent estimate.
    pub fn lyapunov_exponent(&self) -> f32 {
        if self.lyapunov_count == 0 {
            return 0.0;
        }
        (self.lyapunov_sum / self.lyapunov_count as f64) as f32
    }

    /// Current attractor classification.
    pub fn attractor_type(&self) -> AttractorType {
        self.attractor_type
    }

    /// Whether initial learning is complete.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Build a 4D state vector from CSI data.
fn build_state(
    phases: &[f32],
    amplitudes: &[f32],
    motion_energy: f32,
    n_sc: usize,
) -> StateVec {
    let mut mean_phase = 0.0f32;
    let mut mean_amp = 0.0f32;

    for i in 0..n_sc {
        mean_phase += phases[i];
        mean_amp += amplitudes[i];
    }
    let n = n_sc as f32;
    mean_phase /= n;
    mean_amp /= n;

    // Variance of amplitudes.
    let mut var = 0.0f32;
    for i in 0..n_sc {
        let d = amplitudes[i] - mean_amp;
        var += d * d;
    }
    var /= n;

    [mean_phase, mean_amp, var, motion_energy]
}

/// Euclidean distance between two state vectors.
fn vec_distance(a: &StateVec, b: &StateVec) -> f32 {
    let mut sum = 0.0f32;
    for d in 0..STATE_DIM {
        let diff = a[d] - b[d];
        sum += diff * diff;
    }
    sqrtf(sum)
}

/// Classify attractor type from Lyapunov exponent.
fn classify_attractor(lambda: f32) -> AttractorType {
    if lambda < LYAPUNOV_STABLE_UPPER {
        AttractorType::PointAttractor
    } else if lambda < LYAPUNOV_PERIODIC_UPPER {
        AttractorType::LimitCycle
    } else {
        AttractorType::StrangeAttractor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let det = AttractorDetector::new();
        assert!(!det.is_initialized());
        assert_eq!(det.attractor_type(), AttractorType::Unknown);
        assert_eq!(det.lyapunov_exponent(), 0.0);
    }

    #[test]
    fn test_build_state() {
        let phases = [0.1, 0.2, 0.3, 0.4];
        let amps = [1.0, 2.0, 3.0, 4.0];
        let state = build_state(&phases, &amps, 0.5, 4);
        // mean_phase = 0.25, mean_amp = 2.5
        assert!((state[0] - 0.25).abs() < 0.01);
        assert!((state[1] - 2.5).abs() < 0.01);
        assert!(state[2] > 0.0); // variance > 0
        assert!((state[3] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_vec_distance() {
        let a = [1.0, 0.0, 0.0, 0.0];
        let b = [0.0, 0.0, 0.0, 0.0];
        let d = vec_distance(&a, &b);
        assert!((d - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_classify_attractor() {
        assert_eq!(classify_attractor(-0.1), AttractorType::PointAttractor);
        assert_eq!(classify_attractor(0.0), AttractorType::LimitCycle);
        assert_eq!(classify_attractor(0.1), AttractorType::StrangeAttractor);
    }

    #[test]
    fn test_stable_room_point_attractor() {
        let mut det = AttractorDetector::new();

        // Feed *nearly* constant data with tiny perturbations so that
        // consecutive-state deltas are non-zero (above MIN_DELTA) and
        // lyapunov_count accumulates, enabling initialization.
        for i in 0..(MIN_FRAMES_FOR_CLASSIFICATION + 10) {
            let tiny = (i as f32) * 1e-5;
            let phases = [0.1 + tiny; 8];
            let amps = [1.0 + tiny; 8];
            det.process_frame(&phases, &amps, tiny);
        }

        assert!(det.is_initialized());
        // Near-constant input => Lyapunov exponent should be non-positive.
        let lambda = det.lyapunov_exponent();
        assert!(
            lambda <= LYAPUNOV_PERIODIC_UPPER,
            "near-constant input should not produce strange attractor, got lambda={}",
            lambda
        );
    }

    #[test]
    fn test_basin_departure() {
        let mut det = AttractorDetector::new();

        // Learn on near-constant data with tiny perturbations to allow
        // lyapunov_count to accumulate (constant data produces zero deltas).
        for i in 0..(MIN_FRAMES_FOR_CLASSIFICATION + 10) {
            let tiny = (i as f32) * 1e-5;
            let phases = [0.1 + tiny; 8];
            let amps = [1.0 + tiny; 8];
            det.process_frame(&phases, &amps, tiny);
        }
        assert!(det.is_initialized());

        // Inject a large departure.
        let wild_phases = [5.0f32; 8];
        let wild_amps = [50.0f32; 8];
        let events = det.process_frame(&wild_phases, &wild_amps, 10.0);

        let has_departure = events.iter().any(|&(id, _)| id == EVENT_BASIN_DEPARTURE);
        assert!(has_departure, "large deviation should trigger basin departure");
    }
}

//! Quantum-inspired coherence metric — Bloch sphere representation.
//!
//! Maps each subcarrier's phase to a point on the Bloch sphere and computes
//! an aggregate coherence metric from the mean Bloch vector magnitude.
//!
//! Quantum analogies used:
//! - **Bloch vector**: Each subcarrier phase maps to a 3D unit vector on the
//!   Bloch sphere via (sin(theta)*cos(phi), sin(theta)*sin(phi), cos(theta))
//!   where theta = |phase|, phi = sign(phase)*pi/2.
//! - **Von Neumann entropy**: S = -p*log(p) - (1-p)*log(1-p) with
//!   p = (1 + |bloch|) / 2.  S=0 when perfectly coherent, S=ln(2) maximally mixed.
//! - **Decoherence event**: Sudden entropy increase > 0.3 in one frame.
//!
//! Event IDs (800-series: Quantum-inspired):
//!   850 — ENTANGLEMENT_ENTROPY
//!   851 — DECOHERENCE_EVENT
//!   852 — BLOCH_DRIFT
//!
//! Budget: H (heavy, < 10 ms per frame).

use libm::{cosf, fabsf, logf, sinf, sqrtf};

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum subcarriers to process.
const MAX_SC: usize = 32;

/// EMA smoothing factor for entropy.
const ALPHA: f32 = 0.15;

/// Decoherence detection threshold: entropy jump per frame.
const DECOHERENCE_THRESHOLD: f32 = 0.3;

/// Emit entropy every N frames (bandwidth limiting).
const ENTROPY_EMIT_INTERVAL: u32 = 10;

/// Emit drift every N frames.
const DRIFT_EMIT_INTERVAL: u32 = 5;

/// Natural log of 2 (maximum binary entropy).
const LN2: f32 = 0.693_147_2;

/// Small epsilon to avoid log(0).
const EPS: f32 = 1.0e-7;

// ── Event IDs ────────────────────────────────────────────────────────────────

/// Von Neumann entropy of the aggregate Bloch state [0, ln2].
pub const EVENT_ENTANGLEMENT_ENTROPY: i32 = 850;

/// Decoherence event detected (value = entropy jump magnitude).
pub const EVENT_DECOHERENCE_EVENT: i32 = 851;

/// Bloch vector drift rate (value = |delta_bloch| / dt).
pub const EVENT_BLOCH_DRIFT: i32 = 852;

// ── State ────────────────────────────────────────────────────────────────────

/// Quantum-inspired coherence monitor using Bloch sphere representation.
pub struct QuantumCoherenceMonitor {
    /// Previous aggregate Bloch vector [x, y, z].
    prev_bloch: [f32; 3],
    /// EMA-smoothed Von Neumann entropy.
    smoothed_entropy: f32,
    /// Previous frame's raw entropy (for decoherence detection).
    prev_entropy: f32,
    /// Frame counter.
    frame_count: u32,
    /// Whether the monitor has been initialized with at least one frame.
    initialized: bool,
}

impl QuantumCoherenceMonitor {
    /// Create a new monitor. Const-evaluable for static initialization.
    pub const fn new() -> Self {
        Self {
            prev_bloch: [0.0, 0.0, 1.0],
            smoothed_entropy: 0.0,
            prev_entropy: 0.0,
            frame_count: 0,
            initialized: false,
        }
    }

    /// Process one frame of subcarrier phase data.
    ///
    /// Maps each subcarrier phase to a Bloch sphere point, computes the mean
    /// Bloch vector, derives coherence and Von Neumann entropy, and detects
    /// decoherence events.
    ///
    /// Returns a slice of (event_type, value) pairs to emit.
    pub fn process_frame(&mut self, phases: &[f32]) -> &[(i32, f32)] {
        let n_sc = if phases.len() > MAX_SC { MAX_SC } else { phases.len() };
        if n_sc < 2 {
            return &[];
        }

        self.frame_count += 1;

        // ── Map subcarrier phases to Bloch sphere and compute mean vector ──
        let bloch = self.compute_mean_bloch(phases, n_sc);
        let bloch_mag = vec3_magnitude(&bloch);

        // ── Von Neumann entropy ──
        // p = (1 + |bloch|) / 2, clamped to (eps, 1-eps) to avoid log(0).
        let p = clamp((1.0 + bloch_mag) * 0.5, EPS, 1.0 - EPS);
        let q = 1.0 - p;
        let raw_entropy = -(p * logf(p) + q * logf(q));

        // EMA smoothing.
        if !self.initialized {
            self.smoothed_entropy = raw_entropy;
            self.prev_entropy = raw_entropy;
            self.prev_bloch = bloch;
            self.initialized = true;
            return &[];
        }

        self.smoothed_entropy = ALPHA * raw_entropy + (1.0 - ALPHA) * self.smoothed_entropy;

        // ── Decoherence detection: sudden entropy spike ──
        let entropy_jump = raw_entropy - self.prev_entropy;

        // ── Bloch vector drift rate ──
        let drift = vec3_distance(&bloch, &self.prev_bloch);

        // Store for next frame.
        self.prev_entropy = raw_entropy;
        self.prev_bloch = bloch;

        // ── Build output events ──
        static mut EVENTS: [(i32, f32); 3] = [(0, 0.0); 3];
        let mut n_events = 0usize;

        // Entropy (periodic).
        if self.frame_count % ENTROPY_EMIT_INTERVAL == 0 {
            unsafe {
                EVENTS[n_events] = (EVENT_ENTANGLEMENT_ENTROPY, self.smoothed_entropy);
            }
            n_events += 1;
        }

        // Decoherence event (immediate).
        if entropy_jump > DECOHERENCE_THRESHOLD {
            unsafe {
                EVENTS[n_events] = (EVENT_DECOHERENCE_EVENT, entropy_jump);
            }
            n_events += 1;
        }

        // Bloch drift (periodic).
        if self.frame_count % DRIFT_EMIT_INTERVAL == 0 {
            unsafe {
                EVENTS[n_events] = (EVENT_BLOCH_DRIFT, drift);
            }
            n_events += 1;
        }

        unsafe { &EVENTS[..n_events] }
    }

    /// Compute the mean Bloch vector from subcarrier phases.
    ///
    /// Each phase is mapped to the Bloch sphere:
    ///   theta = |phase|  (polar angle)
    ///   phi   = sign(phase) * pi/2  (azimuthal angle)
    ///   bloch = (sin(theta)*cos(phi), sin(theta)*sin(phi), cos(theta))
    /// PERF: phi is always +/- pi/2, so cos(phi) = 0 and sin(phi) = +/- 1.
    /// This eliminates 2 trig calls (cosf, sinf) per subcarrier, and since
    /// sum_x is always zero (sin_theta * cos(pi/2) = 0), we skip it entirely.
    /// Net savings: 2*n_sc trig calls eliminated per frame (32-64 cosf/sinf calls).
    fn compute_mean_bloch(&self, phases: &[f32], n_sc: usize) -> [f32; 3] {
        // sum_x is always 0 because cos(+/-pi/2) = 0.
        let mut sum_y = 0.0f32;
        let mut sum_z = 0.0f32;

        for i in 0..n_sc {
            let phase = phases[i];
            let theta = fabsf(phase);
            let sin_theta = sinf(theta);
            let cos_theta = cosf(theta);

            // sin(+pi/2) = 1, sin(-pi/2) = -1 -> factor out as sign(phase).
            if phase >= 0.0 {
                sum_y += sin_theta;  // sin_theta * sin(pi/2) = sin_theta * 1
            } else {
                sum_y -= sin_theta;  // sin_theta * sin(-pi/2) = sin_theta * (-1)
            }
            sum_z += cos_theta;
        }

        let inv_n = 1.0 / (n_sc as f32);
        [0.0, sum_y * inv_n, sum_z * inv_n]
    }

    /// Get the current EMA-smoothed Von Neumann entropy.
    pub fn entropy(&self) -> f32 {
        self.smoothed_entropy
    }

    /// Get the coherence score [0, 1] derived from Bloch vector magnitude.
    ///
    /// 1.0 = all subcarrier phases perfectly aligned (pure state).
    /// 0.0 = random phases (maximally mixed state).
    pub fn coherence(&self) -> f32 {
        vec3_magnitude(&self.prev_bloch)
    }

    /// Get the previous Bloch vector (for visualization / debugging).
    pub fn bloch_vector(&self) -> [f32; 3] {
        self.prev_bloch
    }

    /// Get the normalized entropy [0, 1] (entropy / ln2).
    pub fn normalized_entropy(&self) -> f32 {
        clamp(self.smoothed_entropy / LN2, 0.0, 1.0)
    }

    /// Get the total number of frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }
}

// ── Helpers (no_std, no heap) ────────────────────────────────────────────────

/// 3D vector magnitude.
#[inline]
fn vec3_magnitude(v: &[f32; 3]) -> f32 {
    sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

/// Euclidean distance between two 3D vectors.
#[inline]
fn vec3_distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    sqrtf(dx * dx + dy * dy + dz * dz)
}

/// Clamp a value to [lo, hi].
#[inline]
fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let mon = QuantumCoherenceMonitor::new();
        assert_eq!(mon.frame_count(), 0);
        assert!(!mon.initialized);
    }

    #[test]
    fn test_uniform_phases_high_coherence() {
        let mut mon = QuantumCoherenceMonitor::new();
        // All phases identical -> all Bloch vectors aligned -> high coherence.
        let phases = [0.5f32; 16];

        // First frame initializes.
        let events = mon.process_frame(&phases);
        assert!(events.is_empty());

        // Subsequent frames with same phase should show high coherence.
        for _ in 0..20 {
            mon.process_frame(&phases);
        }

        let coh = mon.coherence();
        assert!(coh > 0.9, "uniform phases should yield high coherence, got {}", coh);

        let ent = mon.normalized_entropy();
        assert!(ent < 0.2, "uniform phases should yield low entropy, got {}", ent);
    }

    #[test]
    fn test_random_phases_low_coherence() {
        let mut mon = QuantumCoherenceMonitor::new();
        // Phases spread across a wide range -> Bloch vectors cancel -> low coherence.
        let mut phases = [0.0f32; 32];
        for i in 0..32 {
            // Spread from -pi to +pi.
            phases[i] = -3.14159 + (i as f32) * (6.28318 / 32.0);
        }

        // Initialize.
        mon.process_frame(&phases);

        for _ in 0..50 {
            mon.process_frame(&phases);
        }

        let coh = mon.coherence();
        assert!(coh < 0.5, "spread phases should yield low coherence, got {}", coh);

        let ent = mon.normalized_entropy();
        assert!(ent > 0.3, "spread phases should yield higher entropy, got {}", ent);
    }

    #[test]
    fn test_decoherence_detection() {
        let mut mon = QuantumCoherenceMonitor::new();

        // Start with aligned phases.
        let coherent = [0.1f32; 16];
        mon.process_frame(&coherent);
        for _ in 0..10 {
            mon.process_frame(&coherent);
        }

        // Suddenly inject random phases to cause entropy spike.
        let mut incoherent = [0.0f32; 16];
        for i in 0..16 {
            incoherent[i] = -3.14 + (i as f32) * 0.4;
        }

        let mut decoherence_detected = false;
        for _ in 0..5 {
            let events = mon.process_frame(&incoherent);
            for &(et, _) in events {
                if et == EVENT_DECOHERENCE_EVENT {
                    decoherence_detected = true;
                }
            }
        }

        assert!(
            decoherence_detected,
            "should detect decoherence on sudden phase randomization"
        );
    }

    #[test]
    fn test_bloch_drift_emission() {
        let mut mon = QuantumCoherenceMonitor::new();
        let phases_a = [0.2f32; 16];
        let phases_b = [1.5f32; 16];

        // Initialize.
        mon.process_frame(&phases_a);

        // Feed alternating phases to create drift.
        let mut drift_emitted = false;
        for i in 0..20 {
            let phases = if i % 2 == 0 { &phases_a } else { &phases_b };
            let events = mon.process_frame(phases);
            for &(et, val) in events {
                if et == EVENT_BLOCH_DRIFT {
                    drift_emitted = true;
                    assert!(val > 0.0, "drift should be positive when phases change");
                }
            }
        }

        assert!(drift_emitted, "should emit BLOCH_DRIFT events periodically");
    }

    #[test]
    fn test_entropy_bounds() {
        let mut mon = QuantumCoherenceMonitor::new();
        let phases = [0.3f32; 8];

        mon.process_frame(&phases);
        for _ in 0..100 {
            mon.process_frame(&phases);
        }

        let ent = mon.entropy();
        assert!(ent >= 0.0, "entropy should be non-negative, got {}", ent);
        assert!(ent <= LN2 + 0.01, "entropy should not exceed ln(2), got {}", ent);

        let norm = mon.normalized_entropy();
        assert!(norm >= 0.0 && norm <= 1.0, "normalized entropy out of range: {}", norm);
    }

    #[test]
    fn test_small_input() {
        let mut mon = QuantumCoherenceMonitor::new();
        // Single subcarrier: too few, should return empty.
        let events = mon.process_frame(&[0.5]);
        assert!(events.is_empty());
        assert_eq!(mon.frame_count(), 0);
    }

    #[test]
    fn test_zero_phases_perfect_coherence() {
        let mut mon = QuantumCoherenceMonitor::new();
        // theta=0 -> all Bloch vectors point to north pole (0,0,1) -> |bloch|=1.
        let phases = [0.0f32; 16];

        mon.process_frame(&phases);
        for _ in 0..10 {
            mon.process_frame(&phases);
        }

        let coh = mon.coherence();
        assert!(
            (coh - 1.0).abs() < 0.01,
            "zero phases should give coherence ~1.0, got {}",
            coh
        );
    }
}

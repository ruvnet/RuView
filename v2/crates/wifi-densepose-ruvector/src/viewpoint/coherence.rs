//! Coherence gating for environment stability (ADR-031).
//!
//! Phase coherence determines whether the wireless environment is sufficiently
//! stable for a model update. When multipath conditions change rapidly (e.g.
//! doors opening, people entering), phase becomes incoherent and fusion
//! quality degrades. The coherence gate prevents model updates during these
//! transient periods.
//!
//! The core computation is the complex mean of unit phasors:
//!
//! ```text
//! coherence = |mean(exp(j * delta_phi))|
//!           = sqrt((mean(cos(delta_phi)))^2 + (mean(sin(delta_phi)))^2)
//! ```
//!
//! A coherence value near 1.0 indicates consistent phase; near 0.0 indicates
//! random phase (incoherent environment).

// ---------------------------------------------------------------------------
// CoherenceState
// ---------------------------------------------------------------------------

/// Rolling coherence state tracking phase consistency over a sliding window.
///
/// Maintains a circular buffer of phase differences and incrementally updates
/// the coherence estimate as new measurements arrive.
#[derive(Debug, Clone)]
pub struct CoherenceState {
    /// Circular buffer of phase differences (radians).
    phase_diffs: Vec<f32>,
    /// Write position in the circular buffer.
    write_pos: usize,
    /// Number of valid entries in the buffer (may be less than capacity
    /// during warm-up).
    count: usize,
    /// Running sum of cos(phase_diff).
    sum_cos: f64,
    /// Running sum of sin(phase_diff).
    sum_sin: f64,
}

impl CoherenceState {
    /// Create a new coherence state with the given window size.
    ///
    /// # Arguments
    ///
    /// - `window_size`: number of phase measurements to retain. Larger windows
    ///   are more stable but respond more slowly to environment changes.
    ///   Must be at least 1.
    pub fn new(window_size: usize) -> Self {
        let size = window_size.max(1);
        CoherenceState {
            phase_diffs: vec![0.0; size],
            write_pos: 0,
            count: 0,
            sum_cos: 0.0,
            sum_sin: 0.0,
        }
    }

    /// Push a new phase difference measurement into the rolling window.
    ///
    /// If the buffer is full, the oldest measurement is evicted and its
    /// contribution is subtracted from the running sums.
    pub fn push(&mut self, phase_diff: f32) {
        let cap = self.phase_diffs.len();

        // If buffer is full, subtract the evicted entry.
        if self.count == cap {
            let old = self.phase_diffs[self.write_pos];
            self.sum_cos -= old.cos() as f64;
            self.sum_sin -= old.sin() as f64;
        } else {
            self.count += 1;
        }

        // Write new entry.
        self.phase_diffs[self.write_pos] = phase_diff;
        self.sum_cos += phase_diff.cos() as f64;
        self.sum_sin += phase_diff.sin() as f64;

        self.write_pos = (self.write_pos + 1) % cap;
    }

    /// Current coherence value in `[0, 1]`.
    ///
    /// Returns 0.0 if no measurements have been pushed yet.
    pub fn coherence(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        let n = self.count as f64;
        let mean_cos = self.sum_cos / n;
        let mean_sin = self.sum_sin / n;
        (mean_cos * mean_cos + mean_sin * mean_sin).sqrt() as f32
    }

    /// Number of measurements currently in the buffer.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` if no measurements have been pushed.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Window capacity.
    pub fn capacity(&self) -> usize {
        self.phase_diffs.len()
    }

    /// Reset the coherence state, clearing all measurements.
    pub fn reset(&mut self) {
        self.write_pos = 0;
        self.count = 0;
        self.sum_cos = 0.0;
        self.sum_sin = 0.0;
    }
}

// ---------------------------------------------------------------------------
// CoherenceGate
// ---------------------------------------------------------------------------

/// Coherence gate that controls model updates based on phase stability.
///
/// Only allows model updates when the coherence exceeds a configurable
/// threshold. Provides hysteresis to avoid rapid gate toggling near the
/// threshold boundary.
#[derive(Debug, Clone)]
pub struct CoherenceGate {
    /// Coherence threshold for opening the gate.
    pub threshold: f32,
    /// Hysteresis band: gate opens at `threshold` and closes at
    /// `threshold - hysteresis`.
    pub hysteresis: f32,
    /// Current gate state: `true` = open (updates allowed).
    gate_open: bool,
    /// Total number of gate evaluations.
    total_evaluations: u64,
    /// Number of times the gate was open.
    open_count: u64,
}

impl CoherenceGate {
    /// Create a new coherence gate with the given threshold.
    ///
    /// # Arguments
    ///
    /// - `threshold`: coherence level required for the gate to open (typically 0.7).
    /// - `hysteresis`: band below the threshold where the gate stays in its
    ///   current state (typically 0.05).
    pub fn new(threshold: f32, hysteresis: f32) -> Self {
        CoherenceGate {
            threshold: threshold.clamp(0.0, 1.0),
            hysteresis: hysteresis.clamp(0.0, threshold),
            gate_open: false,
            total_evaluations: 0,
            open_count: 0,
        }
    }

    /// Create a gate with default parameters (threshold=0.7, hysteresis=0.05).
    pub fn default_params() -> Self {
        Self::new(0.7, 0.05)
    }

    /// Evaluate the gate against the current coherence value.
    ///
    /// Returns `true` if the gate is open (model update allowed).
    pub fn evaluate(&mut self, coherence: f32) -> bool {
        self.total_evaluations += 1;

        if self.gate_open {
            // Gate is open: close if coherence drops below threshold - hysteresis.
            if coherence < self.threshold - self.hysteresis {
                self.gate_open = false;
            }
        } else {
            // Gate is closed: open if coherence exceeds threshold.
            if coherence >= self.threshold {
                self.gate_open = true;
            }
        }

        if self.gate_open {
            self.open_count += 1;
        }

        self.gate_open
    }

    /// Whether the gate is currently open.
    pub fn is_open(&self) -> bool {
        self.gate_open
    }

    /// Fraction of evaluations where the gate was open.
    pub fn duty_cycle(&self) -> f32 {
        if self.total_evaluations == 0 {
            return 0.0;
        }
        self.open_count as f32 / self.total_evaluations as f32
    }

    /// Reset the gate state and counters.
    pub fn reset(&mut self) {
        self.gate_open = false;
        self.total_evaluations = 0;
        self.open_count = 0;
    }
}

/// Stateless coherence gate function matching the ADR-031 specification.
///
/// Computes the complex mean of unit phasors from the given phase differences
/// and returns `true` when coherence exceeds the threshold.
///
/// # Arguments
///
/// - `phase_diffs`: delta-phi over T recent frames (radians).
/// - `threshold`: coherence threshold (typically 0.7).
///
/// # Returns
///
/// `true` if the phase coherence exceeds the threshold.
pub fn coherence_gate(phase_diffs: &[f32], threshold: f32) -> bool {
    if phase_diffs.is_empty() {
        return false;
    }
    let (sum_cos, sum_sin) = phase_diffs
        .iter()
        .fold((0.0_f32, 0.0_f32), |(c, s), &dp| {
            (c + dp.cos(), s + dp.sin())
        });
    let n = phase_diffs.len() as f32;
    let coherence = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
    coherence > threshold
}

/// Compute the raw coherence value from phase differences.
///
/// Returns a value in `[0, 1]` where 1.0 = perfectly coherent phase.
pub fn compute_coherence(phase_diffs: &[f32]) -> f32 {
    if phase_diffs.is_empty() {
        return 0.0;
    }
    let (sum_cos, sum_sin) = phase_diffs
        .iter()
        .fold((0.0_f32, 0.0_f32), |(c, s), &dp| {
            (c + dp.cos(), s + dp.sin())
        });
    let n = phase_diffs.len() as f32;
    ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coherent_phase_returns_high_value() {
        // All phase diffs are the same -> coherence ~ 1.0
        let phase_diffs = vec![0.5_f32; 100];
        let c = compute_coherence(&phase_diffs);
        assert!(c > 0.99, "identical phases should give coherence ~ 1.0, got {c}");
    }

    #[test]
    fn random_phase_returns_low_value() {
        // Uniformly spaced phases around the circle -> coherence ~ 0.0
        let n = 1000;
        let phase_diffs: Vec<f32> = (0..n)
            .map(|i| 2.0 * std::f32::consts::PI * i as f32 / n as f32)
            .collect();
        let c = compute_coherence(&phase_diffs);
        assert!(c < 0.05, "uniformly spread phases should give coherence ~ 0.0, got {c}");
    }

    #[test]
    fn coherence_gate_opens_above_threshold() {
        let coherent = vec![0.3_f32; 50]; // same phase -> high coherence
        assert!(coherence_gate(&coherent, 0.7));
    }

    #[test]
    fn coherence_gate_closed_below_threshold() {
        let n = 500;
        let incoherent: Vec<f32> = (0..n)
            .map(|i| 2.0 * std::f32::consts::PI * i as f32 / n as f32)
            .collect();
        assert!(!coherence_gate(&incoherent, 0.7));
    }

    #[test]
    fn coherence_gate_empty_returns_false() {
        assert!(!coherence_gate(&[], 0.5));
    }

    #[test]
    fn coherence_state_rolling_window() {
        let mut state = CoherenceState::new(10);
        // Push coherent measurements.
        for _ in 0..10 {
            state.push(1.0);
        }
        let c1 = state.coherence();
        assert!(c1 > 0.9, "coherent window should give high coherence");

        // Push incoherent measurements to replace the window.
        for i in 0..10 {
            state.push(i as f32 * 0.628);
        }
        let c2 = state.coherence();
        assert!(c2 < c1, "incoherent updates should reduce coherence");
    }

    #[test]
    fn coherence_state_empty_returns_zero() {
        let state = CoherenceState::new(10);
        assert_eq!(state.coherence(), 0.0);
        assert!(state.is_empty());
    }

    #[test]
    fn gate_hysteresis_prevents_toggling() {
        let mut gate = CoherenceGate::new(0.7, 0.1);
        // Open the gate.
        assert!(gate.evaluate(0.8));
        assert!(gate.is_open());

        // Coherence drops to 0.65 (below threshold but within hysteresis band).
        assert!(gate.evaluate(0.65));
        assert!(gate.is_open(), "gate should stay open within hysteresis band");

        // Coherence drops below hysteresis boundary (0.7 - 0.1 = 0.6).
        assert!(!gate.evaluate(0.55));
        assert!(!gate.is_open(), "gate should close below hysteresis boundary");
    }

    #[test]
    fn gate_duty_cycle_tracks_correctly() {
        let mut gate = CoherenceGate::new(0.5, 0.0);
        gate.evaluate(0.6); // open
        gate.evaluate(0.6); // open
        gate.evaluate(0.3); // close
        gate.evaluate(0.3); // close
        let duty = gate.duty_cycle();
        assert!(
            (duty - 0.5).abs() < 1e-5,
            "duty cycle should be 0.5, got {duty}"
        );
    }

    #[test]
    fn gate_reset_clears_state() {
        let mut gate = CoherenceGate::new(0.5, 0.0);
        gate.evaluate(0.6);
        assert!(gate.is_open());
        gate.reset();
        assert!(!gate.is_open());
        assert_eq!(gate.duty_cycle(), 0.0);
    }

    #[test]
    fn coherence_state_push_and_len() {
        let mut state = CoherenceState::new(5);
        assert_eq!(state.len(), 0);
        state.push(0.1);
        state.push(0.2);
        assert_eq!(state.len(), 2);
        // Fill past capacity.
        for i in 0..10 {
            state.push(i as f32 * 0.1);
        }
        assert_eq!(state.len(), 5, "count should be capped at window size");
    }
}

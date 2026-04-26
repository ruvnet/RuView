//! Meta-learning parameter self-optimization with safety constraints.
//!
//! ADR-041 adaptive learning module — Event IDs 740-743.
//!
//! Maintains 8 tunable runtime parameters (thresholds for presence, motion,
//! coherence, gesture DTW, etc.) and optimizes them via hill-climbing on a
//! performance score derived from event feedback.
//!
//! Performance score = true_positive_rate - 2 * false_positive_rate
//!   (penalizes false positives more heavily than missing true positives)
//!
//! Optimization loop (runs on_timer, not per-frame):
//!   1. Perturb one parameter by +/- step_size
//!   2. Evaluate performance score over the next evaluation window
//!   3. Keep change if score improved, revert if not
//!   4. Safety: never exceed min/max bounds, rollback all changes if 3
//!      consecutive degradations occur
//!
//! Budget: S (standard, < 5 ms — runs on timer, not per-frame).

/// Number of tunable parameters.
const NUM_PARAMS: usize = 8;

/// Maximum consecutive failures before safety rollback.
const MAX_CONSECUTIVE_FAILURES: u8 = 3;

/// Minimum evaluation window (timer ticks) before scoring a perturbation.
const EVAL_WINDOW: u16 = 10;

/// Default parameter step size (fraction of range).
const DEFAULT_STEP_FRAC: f32 = 0.05;

// ── Event IDs (740-series: Meta-learning) ────────────────────────────────────

pub const EVENT_PARAM_ADJUSTED: i32 = 740;
pub const EVENT_ADAPTATION_SCORE: i32 = 741;
pub const EVENT_ROLLBACK_TRIGGERED: i32 = 742;
pub const EVENT_META_LEVEL: i32 = 743;

/// One tunable parameter with bounds and step size.
#[derive(Clone, Copy)]
struct TunableParam {
    /// Current value.
    value: f32,
    /// Minimum allowed value.
    min_bound: f32,
    /// Maximum allowed value.
    max_bound: f32,
    /// Perturbation step size.
    step_size: f32,
    /// Value before the current perturbation (for revert).
    prev_value: f32,
}

impl TunableParam {
    const fn new(value: f32, min_bound: f32, max_bound: f32, step_size: f32) -> Self {
        Self {
            value,
            min_bound,
            max_bound,
            step_size,
            prev_value: value,
        }
    }

    /// Clamp value to bounds.
    fn clamp(&mut self) {
        if self.value < self.min_bound {
            self.value = self.min_bound;
        }
        if self.value > self.max_bound {
            self.value = self.max_bound;
        }
    }
}

/// Optimization phase state.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OptPhase {
    /// Baseline measurement — collecting score before perturbation.
    Baseline,
    /// A parameter has been perturbed; evaluating the result.
    Evaluating,
}

/// Meta-learning parameter optimizer.
pub struct MetaAdapter {
    /// Tunable parameters.
    params: [TunableParam; NUM_PARAMS],

    /// Snapshot of all parameter values before any perturbation chain
    /// (used for safety rollback).
    rollback_snapshot: [f32; NUM_PARAMS],

    /// Current optimization phase.
    phase: OptPhase,
    /// Index of the parameter currently being perturbed.
    current_param: usize,
    /// Direction of current perturbation (+1 or -1).
    perturb_direction: i8,

    /// Baseline performance score (before perturbation).
    baseline_score: f32,
    /// Current accumulated performance score.
    current_score: f32,

    /// Event feedback accumulators (reset each evaluation window).
    true_positives: u16,
    false_positives: u16,
    total_events: u16,

    /// Ticks elapsed in the current evaluation window.
    eval_ticks: u16,

    /// Consecutive failed perturbations (score did not improve).
    consecutive_failures: u8,
    /// Total perturbation iterations.
    iteration_count: u32,
    /// Total successful adaptations.
    success_count: u32,

    /// Meta-level: increases with each full parameter sweep, represents
    /// how many optimization rounds have completed.
    meta_level: u16,
    /// Counter within a sweep (0..NUM_PARAMS).
    sweep_idx: usize,
}

impl MetaAdapter {
    /// Create a new meta-adapter with default parameter configuration.
    ///
    /// Default parameters (indices correspond to sensing thresholds):
    ///   0: presence_threshold      (0.05,  range 0.01-0.5)
    ///   1: motion_threshold        (0.10,  range 0.02-1.0)
    ///   2: coherence_threshold     (0.70,  range 0.3-0.99)
    ///   3: gesture_dtw_threshold   (2.50,  range 0.5-5.0)
    ///   4: anomaly_energy_ratio    (50.0,  range 10.0-200.0)
    ///   5: zone_occupancy_thresh   (0.02,  range 0.005-0.1)
    ///   6: vital_apnea_seconds     (20.0,  range 10.0-60.0)
    ///   7: intrusion_sensitivity   (0.30,  range 0.05-0.9)
    pub const fn new() -> Self {
        Self {
            params: [
                TunableParam::new(0.05, 0.01, 0.50, 0.01),
                TunableParam::new(0.10, 0.02, 1.00, 0.02),
                TunableParam::new(0.70, 0.30, 0.99, 0.02),
                TunableParam::new(2.50, 0.50, 5.00, 0.20),
                TunableParam::new(50.0, 10.0, 200.0, 5.0),
                TunableParam::new(0.02, 0.005, 0.10, 0.005),
                TunableParam::new(20.0, 10.0, 60.0, 2.0),
                TunableParam::new(0.30, 0.05, 0.90, 0.03),
            ],
            rollback_snapshot: [0.05, 0.10, 0.70, 2.50, 50.0, 0.02, 20.0, 0.30],
            phase: OptPhase::Baseline,
            current_param: 0,
            perturb_direction: 1,
            baseline_score: 0.0,
            current_score: 0.0,
            true_positives: 0,
            false_positives: 0,
            total_events: 0,
            eval_ticks: 0,
            consecutive_failures: 0,
            iteration_count: 0,
            success_count: 0,
            meta_level: 0,
            sweep_idx: 0,
        }
    }

    /// Report a true positive event (correct detection confirmed by context).
    pub fn report_true_positive(&mut self) {
        self.true_positives = self.true_positives.saturating_add(1);
        self.total_events = self.total_events.saturating_add(1);
    }

    /// Report a false positive event (detection that should not have fired).
    pub fn report_false_positive(&mut self) {
        self.false_positives = self.false_positives.saturating_add(1);
        self.total_events = self.total_events.saturating_add(1);
    }

    /// Report a generic event (for total count normalization).
    pub fn report_event(&mut self) {
        self.total_events = self.total_events.saturating_add(1);
    }

    /// Get the current value of a parameter by index.
    pub fn get_param(&self, idx: usize) -> f32 {
        if idx < NUM_PARAMS {
            self.params[idx].value
        } else {
            0.0
        }
    }

    /// Called on timer (typically 1 Hz). Drives the optimization loop.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn on_timer(&mut self) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        self.eval_ticks += 1;

        // ── Compute current performance score ────────────────────────────
        let score = self.compute_score();
        self.current_score = score;

        match self.phase {
            OptPhase::Baseline => {
                if self.eval_ticks >= EVAL_WINDOW {
                    // Record baseline score and apply perturbation.
                    self.baseline_score = score;
                    self.apply_perturbation();
                    self.reset_accumulators();
                    self.phase = OptPhase::Evaluating;
                }
            }
            OptPhase::Evaluating => {
                if self.eval_ticks >= EVAL_WINDOW {
                    self.iteration_count += 1;

                    let improved = score > self.baseline_score;

                    if improved {
                        // Keep the perturbation.
                        self.consecutive_failures = 0;
                        self.success_count += 1;

                        unsafe {
                            EVENTS[n_ev] = (
                                EVENT_PARAM_ADJUSTED,
                                self.current_param as f32
                                    + self.params[self.current_param].value / 1000.0,
                            );
                            n_ev += 1;
                            EVENTS[n_ev] = (EVENT_ADAPTATION_SCORE, score);
                            n_ev += 1;
                        }
                    } else {
                        // Revert the perturbation.
                        self.params[self.current_param].value =
                            self.params[self.current_param].prev_value;
                        self.consecutive_failures += 1;
                    }

                    // ── Safety rollback ──────────────────────────────────
                    if self.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        self.safety_rollback();
                        unsafe {
                            EVENTS[n_ev] = (EVENT_ROLLBACK_TRIGGERED, self.meta_level as f32);
                            n_ev += 1;
                        }
                    }

                    // ── Advance to next parameter ────────────────────────
                    self.advance_sweep();
                    self.reset_accumulators();
                    self.phase = OptPhase::Baseline;

                    // ── Emit meta level periodically ─────────────────────
                    if self.sweep_idx == 0 && n_ev < 4 {
                        unsafe {
                            EVENTS[n_ev] = (EVENT_META_LEVEL, self.meta_level as f32);
                            n_ev += 1;
                        }
                    }
                }
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Compute the performance score from accumulated feedback.
    fn compute_score(&self) -> f32 {
        if self.total_events == 0 {
            return 0.0;
        }
        let total = self.total_events as f32;
        let tp_rate = self.true_positives as f32 / total;
        let fp_rate = self.false_positives as f32 / total;
        tp_rate - 2.0 * fp_rate
    }

    /// Apply a perturbation to the current parameter.
    fn apply_perturbation(&mut self) {
        let p = &mut self.params[self.current_param];
        p.prev_value = p.value;

        let delta = p.step_size * self.perturb_direction as f32;
        p.value += delta;
        p.clamp();

        // Alternate perturbation direction each iteration.
        self.perturb_direction = if self.perturb_direction > 0 { -1 } else { 1 };
    }

    /// Advance to the next parameter in the sweep.
    fn advance_sweep(&mut self) {
        self.sweep_idx += 1;
        if self.sweep_idx >= NUM_PARAMS {
            self.sweep_idx = 0;
            self.meta_level = self.meta_level.saturating_add(1);
            // Take a new rollback snapshot after a successful sweep.
            self.snapshot_params();
        }
        self.current_param = self.sweep_idx;
    }

    /// Reset evaluation accumulators for the next window.
    fn reset_accumulators(&mut self) {
        self.true_positives = 0;
        self.false_positives = 0;
        self.total_events = 0;
        self.eval_ticks = 0;
    }

    /// Take a snapshot of current parameter values for rollback.
    fn snapshot_params(&mut self) {
        for i in 0..NUM_PARAMS {
            self.rollback_snapshot[i] = self.params[i].value;
        }
    }

    /// Safety rollback: restore all parameters to the last known-good snapshot.
    fn safety_rollback(&mut self) {
        for i in 0..NUM_PARAMS {
            self.params[i].value = self.rollback_snapshot[i];
            self.params[i].prev_value = self.rollback_snapshot[i];
        }
        self.consecutive_failures = 0;
        // Reset sweep to start fresh.
        self.sweep_idx = 0;
        self.current_param = 0;
    }

    /// Total number of optimization iterations completed.
    pub fn iteration_count(&self) -> u32 {
        self.iteration_count
    }

    /// Total number of successful parameter adaptations.
    pub fn success_count(&self) -> u32 {
        self.success_count
    }

    /// Current meta-level (number of complete sweeps).
    pub fn meta_level(&self) -> u16 {
        self.meta_level
    }

    /// Current consecutive failure count.
    pub fn consecutive_failures(&self) -> u8 {
        self.consecutive_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let ma = MetaAdapter::new();
        assert_eq!(ma.iteration_count(), 0);
        assert_eq!(ma.success_count(), 0);
        assert_eq!(ma.meta_level(), 0);
        assert_eq!(ma.consecutive_failures(), 0);
    }

    #[test]
    fn test_default_params() {
        let ma = MetaAdapter::new();
        assert!((ma.get_param(0) - 0.05).abs() < 0.001); // presence_threshold
        assert!((ma.get_param(1) - 0.10).abs() < 0.001); // motion_threshold
        assert!((ma.get_param(2) - 0.70).abs() < 0.001); // coherence_threshold
        assert!((ma.get_param(3) - 2.50).abs() < 0.001); // gesture_dtw_threshold
        assert!((ma.get_param(7) - 0.30).abs() < 0.001); // intrusion_sensitivity
        assert_eq!(ma.get_param(99), 0.0); // out-of-range
    }

    #[test]
    fn test_score_computation() {
        let mut ma = MetaAdapter::new();
        // 8 TP, 1 FP, 1 generic event = 10 total.
        for _ in 0..8 {
            ma.report_true_positive();
        }
        ma.report_false_positive();
        ma.report_event();

        let score = ma.compute_score();
        // tp_rate = 8/10 = 0.8, fp_rate = 1/10 = 0.1
        // score = 0.8 - 2*0.1 = 0.6
        assert!((score - 0.6).abs() < 0.01, "score should be ~0.6, got {}", score);
    }

    #[test]
    fn test_score_all_false_positives() {
        let mut ma = MetaAdapter::new();
        for _ in 0..10 {
            ma.report_false_positive();
        }
        let score = ma.compute_score();
        // tp_rate = 0, fp_rate = 1.0 => score = -2.0
        assert!(score < -1.0, "all-FP score should be very negative");
    }

    #[test]
    fn test_score_empty() {
        let ma = MetaAdapter::new();
        assert_eq!(ma.compute_score(), 0.0);
    }

    #[test]
    fn test_param_clamping() {
        let mut p = TunableParam::new(0.5, 0.1, 0.9, 0.1);
        p.value = 1.5;
        p.clamp();
        assert!((p.value - 0.9).abs() < 0.001);

        p.value = -0.5;
        p.clamp();
        assert!((p.value - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_optimization_cycle() {
        let mut ma = MetaAdapter::new();

        // Run baseline phase.
        for _ in 0..EVAL_WINDOW {
            ma.report_true_positive();
            ma.on_timer();
        }
        // Should now be in Evaluating phase.
        assert_eq!(ma.phase, OptPhase::Evaluating);

        // Run evaluation phase with good feedback.
        for _ in 0..EVAL_WINDOW {
            ma.report_true_positive();
            ma.on_timer();
        }
        // Should have completed one iteration.
        assert_eq!(ma.iteration_count(), 1);
    }

    #[test]
    fn test_safety_rollback() {
        let mut ma = MetaAdapter::new();
        let original_val = ma.get_param(0);

        // Manually trigger consecutive failures.
        ma.consecutive_failures = MAX_CONSECUTIVE_FAILURES;
        ma.safety_rollback();

        assert_eq!(ma.consecutive_failures(), 0);
        assert!((ma.get_param(0) - original_val).abs() < 0.001);
    }

    #[test]
    fn test_full_sweep_increments_meta_level() {
        let mut ma = MetaAdapter::new();
        ma.sweep_idx = NUM_PARAMS - 1;
        ma.advance_sweep();
        assert_eq!(ma.meta_level(), 1);
        assert_eq!(ma.sweep_idx, 0);
    }
}

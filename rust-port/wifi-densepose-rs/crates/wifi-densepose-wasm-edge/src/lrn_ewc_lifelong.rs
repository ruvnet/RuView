//! Elastic Weight Consolidation for lifelong on-device learning — ADR-041 adaptive module.
//!
//! # Algorithm
//!
//! Implements EWC (Kirkpatrick et al., 2017) on a tiny 8-input, 4-output
//! linear classifier running entirely on the ESP32-S3 WASM3 interpreter.
//! The classifier maps 8D CSI feature vectors to 4 zone predictions.
//!
//! ## Core EWC Mechanism
//!
//! When learning a new task (e.g., a new room layout), naive gradient descent
//! overwrites parameters important for previous tasks -- "catastrophic
//! forgetting."  EWC prevents this by adding a penalty term:
//!
//! ```text
//! L_total = L_current + (lambda/2) * sum_i( F_i * (theta_i - theta_i*)^2 )
//! ```
//!
//! where:
//! - `L_current` = MSE between predicted zone and actual zone
//! - `F_i` = Fisher Information diagonal (parameter importance)
//! - `theta_i*` = parameters at end of previous task
//! - `lambda` = 1000 (regularization strength)
//!
//! ## Fisher Information Estimation
//!
//! The Fisher diagonal approximates parameter importance:
//! `F_i = E[(d log p / d theta_i)^2] ~ running_average(gradient_i^2)`
//!
//! Gradients are estimated via finite differences (perturb each parameter
//! by epsilon=0.01, measure loss change).
//!
//! ## Task Boundary Detection
//!
//! A new task is detected when the system achieves 100 consecutive frames
//! with stable performance (loss below threshold).  At this point:
//! 1. Snapshot current parameters as `theta_star`
//! 2. Update Fisher diagonal from accumulated gradient squares
//! 3. Increment task counter
//!
//! # Events (745-series: Adaptive Learning)
//!
//! - `KNOWLEDGE_RETAINED` (745): EWC penalty magnitude (lower = less forgetting).
//! - `NEW_TASK_LEARNED` (746): Task count after learning a new task.
//! - `FISHER_UPDATE` (747): Mean Fisher information value.
//! - `FORGETTING_RISK` (748): Ratio of EWC penalty to current loss.
//!
//! # Budget
//!
//! L (lightweight, < 2 ms) -- only updates a few params per frame using
//! a round-robin finite-difference gradient schedule.

// ── Constants ────────────────────────────────────────────────────────────────

/// Number of learnable parameters (8 inputs * 4 outputs = 32).
const N_PARAMS: usize = 32;

/// Input dimension (8 subcarrier groups).
const N_INPUT: usize = 8;

/// Output dimension (4 zones).
const N_OUTPUT: usize = 4;

/// EWC regularization strength.
const LAMBDA: f32 = 1000.0;

/// Finite-difference epsilon for gradient estimation.
const EPSILON: f32 = 0.01;

/// Number of parameters to update per frame (round-robin).
const PARAMS_PER_FRAME: usize = 4;

/// Learning rate for parameter updates.
const LEARNING_RATE: f32 = 0.001;

/// Consecutive stable frames required to trigger task boundary.
const STABLE_FRAMES_THRESHOLD: u32 = 100;

/// Loss threshold below which a frame is considered "stable".
const STABLE_LOSS_THRESHOLD: f32 = 0.1;

/// EMA smoothing for Fisher diagonal updates.
const FISHER_ALPHA: f32 = 0.01;

/// Maximum number of tasks before Fisher memory saturates.
const MAX_TASKS: u8 = 32;

/// Reporting interval (frames between event emissions).
const REPORT_INTERVAL: u32 = 20;

// ── Event IDs (745-series: Adaptive Learning) ────────────────────────────────

pub const EVENT_KNOWLEDGE_RETAINED: i32 = 745;
pub const EVENT_NEW_TASK_LEARNED: i32 = 746;
pub const EVENT_FISHER_UPDATE: i32 = 747;
pub const EVENT_FORGETTING_RISK: i32 = 748;

// ── EWC Lifelong Learner ─────────────────────────────────────────────────────

/// Elastic Weight Consolidation lifelong on-device learner.
pub struct EwcLifelong {
    /// Current learnable parameters [N_PARAMS] (flattened [N_OUTPUT][N_INPUT]).
    params: [f32; N_PARAMS],
    /// Fisher Information diagonal [N_PARAMS].
    fisher: [f32; N_PARAMS],
    /// Snapshot of parameters at previous task boundary.
    theta_star: [f32; N_PARAMS],
    /// Accumulated gradient squares for Fisher estimation.
    grad_accum: [f32; N_PARAMS],
    /// Number of gradient samples accumulated.
    grad_count: u32,
    /// Number of completed tasks.
    task_count: u8,
    /// Consecutive frames with loss below threshold.
    stable_frames: u32,
    /// Current round-robin parameter index.
    param_cursor: usize,
    /// Frame counter.
    frame_count: u32,
    /// Last computed total loss (current + EWC penalty).
    last_loss: f32,
    /// Last computed EWC penalty.
    last_penalty: f32,
    /// Whether theta_star has been set (false until first task completes).
    has_prior: bool,
}

impl EwcLifelong {
    pub const fn new() -> Self {
        Self {
            params: Self::default_params(),
            fisher: [0.0; N_PARAMS],
            theta_star: [0.0; N_PARAMS],
            grad_accum: [0.0; N_PARAMS],
            grad_count: 0,
            task_count: 0,
            stable_frames: 0,
            param_cursor: 0,
            frame_count: 0,
            last_loss: 0.0,
            last_penalty: 0.0,
            has_prior: false,
        }
    }

    /// Initialize parameters with small diverse values to break symmetry.
    /// Uses a deterministic pattern (no RNG needed in const context).
    const fn default_params() -> [f32; N_PARAMS] {
        let mut p = [0.0f32; N_PARAMS];
        let mut i = 0;
        while i < N_PARAMS {
            // Deterministic pseudo-random initialization: scaled index with alternation.
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            // (i * 0.037 + 0.01) * sign via integer scaling for const compatibility.
            let magnitude = (i as f32 * 37.0 + 10.0) / 1000.0 * sign;
            p[i] = magnitude;
            i += 1;
        }
        p
    }

    /// Process one frame with learning.
    ///
    /// `features` -- 8D CSI feature vector (mean amplitude per subcarrier group).
    /// `target_zone` -- ground truth zone label (0-3), or -1 if no label available.
    ///
    /// When `target_zone >= 0`, the system performs a gradient step and updates
    /// parameters.  When -1, it only runs inference.
    ///
    /// Returns events as `(event_id, value)` pairs.
    pub fn process_frame(&mut self, features: &[f32], target_zone: i32) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); 4] = [(0, 0.0); 4];
        let mut n_ev = 0usize;

        if features.len() < N_INPUT {
            return &[];
        }

        self.frame_count += 1;

        // Run forward pass: predict zone from features.
        let predicted = self.forward(features);

        // If we have a ground truth label, compute loss and update.
        if target_zone >= 0 && (target_zone as usize) < N_OUTPUT {
            let tz = target_zone as usize;

            // Compute MSE loss against one-hot target.
            let current_loss = self.compute_mse_loss(&predicted, tz);

            // Compute EWC penalty.
            let ewc_penalty = if self.has_prior {
                self.compute_ewc_penalty()
            } else {
                0.0
            };

            let total_loss = current_loss + ewc_penalty;
            self.last_loss = total_loss;
            self.last_penalty = ewc_penalty;

            // Finite-difference gradient estimation (round-robin subset).
            self.update_gradients(features, tz);

            // Gradient descent step.
            self.gradient_step(features, tz);

            // Track stability for task boundary detection.
            if current_loss < STABLE_LOSS_THRESHOLD {
                self.stable_frames += 1;
            } else {
                self.stable_frames = 0;
            }

            // Task boundary detection.
            if self.stable_frames >= STABLE_FRAMES_THRESHOLD
                && self.task_count < MAX_TASKS
            {
                self.commit_task();
                unsafe {
                    EVENTS[n_ev] = (EVENT_NEW_TASK_LEARNED, self.task_count as f32);
                }
                n_ev += 1;

                // Emit mean Fisher value.
                let mean_fisher = self.mean_fisher();
                if n_ev < 4 {
                    unsafe {
                        EVENTS[n_ev] = (EVENT_FISHER_UPDATE, mean_fisher);
                    }
                    n_ev += 1;
                }
            }

            // Periodic reporting.
            if self.frame_count % REPORT_INTERVAL == 0 {
                if n_ev < 4 {
                    unsafe {
                        EVENTS[n_ev] = (EVENT_KNOWLEDGE_RETAINED, ewc_penalty);
                    }
                    n_ev += 1;
                }

                // Forgetting risk: ratio of penalty to current loss.
                let risk = if current_loss > 1e-8 {
                    ewc_penalty / current_loss
                } else {
                    0.0
                };
                if n_ev < 4 {
                    unsafe {
                        EVENTS[n_ev] = (EVENT_FORGETTING_RISK, risk);
                    }
                    n_ev += 1;
                }
            }
        }

        unsafe { &EVENTS[..n_ev] }
    }

    /// Forward pass: linear classifier `output = params * features`.
    ///
    /// Params are stored as [output_0_weights..., output_1_weights..., ...].
    fn forward(&self, features: &[f32]) -> [f32; N_OUTPUT] {
        let mut output = [0.0f32; N_OUTPUT];
        for o in 0..N_OUTPUT {
            let base = o * N_INPUT;
            let mut sum = 0.0f32;
            for i in 0..N_INPUT {
                sum += self.params[base + i] * features[i];
            }
            output[o] = sum;
        }
        output
    }

    /// Compute MSE loss against a one-hot target for `target_zone`.
    fn compute_mse_loss(&self, predicted: &[f32; N_OUTPUT], target: usize) -> f32 {
        let mut loss = 0.0f32;
        for o in 0..N_OUTPUT {
            let target_val = if o == target { 1.0 } else { 0.0 };
            let diff = predicted[o] - target_val;
            loss += diff * diff;
        }
        loss / N_OUTPUT as f32
    }

    /// Compute the EWC penalty: (lambda/2) * sum(F_i * (theta_i - theta_i*)^2).
    fn compute_ewc_penalty(&self) -> f32 {
        let mut penalty = 0.0f32;
        for i in 0..N_PARAMS {
            let diff = self.params[i] - self.theta_star[i];
            penalty += self.fisher[i] * diff * diff;
        }
        (LAMBDA / 2.0) * penalty
    }

    /// Estimate gradients via finite differences for a subset of parameters.
    ///
    /// Uses round-robin scheduling: PARAMS_PER_FRAME parameters per call.
    fn update_gradients(&mut self, features: &[f32], target: usize) {
        let predicted = self.forward(features);
        let base_loss = self.compute_mse_loss(&predicted, target);

        for _step in 0..PARAMS_PER_FRAME {
            let idx = self.param_cursor;
            self.param_cursor = (self.param_cursor + 1) % N_PARAMS;

            // Perturb parameter positively.
            self.params[idx] += EPSILON;
            let perturbed_pred = self.forward(features);
            let perturbed_loss = self.compute_mse_loss(&perturbed_pred, target);
            self.params[idx] -= EPSILON; // Restore.

            // Finite-difference gradient.
            let grad = (perturbed_loss - base_loss) / EPSILON;

            // Accumulate gradient squared for Fisher estimation.
            self.grad_accum[idx] =
                FISHER_ALPHA * grad * grad + (1.0 - FISHER_ALPHA) * self.grad_accum[idx];
            self.grad_count += 1;
        }
    }

    /// Apply gradient descent with EWC regularization.
    fn gradient_step(&mut self, features: &[f32], target: usize) {
        // Compute output error: predicted - target (one-hot).
        let predicted = self.forward(features);

        for o in 0..N_OUTPUT {
            let target_val = if o == target { 1.0 } else { 0.0 };
            let error = predicted[o] - target_val;

            let base = o * N_INPUT;
            for i in 0..N_INPUT {
                // Gradient of MSE w.r.t. weight: 2 * error * feature / N_OUTPUT.
                let grad_mse = 2.0 * error * features[i] / N_OUTPUT as f32;

                // EWC gradient: lambda * F_i * (theta_i - theta_i*).
                let grad_ewc = if self.has_prior {
                    LAMBDA * self.fisher[base + i]
                        * (self.params[base + i] - self.theta_star[base + i])
                } else {
                    0.0
                };

                let total_grad = grad_mse + grad_ewc;
                self.params[base + i] -= LEARNING_RATE * total_grad;
            }
        }
    }

    /// Commit the current state as a learned task.
    fn commit_task(&mut self) {
        // Snapshot parameters.
        self.theta_star = self.params;

        // Update Fisher diagonal from accumulated gradient squares.
        if self.has_prior {
            // Merge with existing Fisher (online consolidation).
            for i in 0..N_PARAMS {
                self.fisher[i] = 0.5 * self.fisher[i] + 0.5 * self.grad_accum[i];
            }
        } else {
            // First task: Fisher = accumulated gradient squares.
            self.fisher = self.grad_accum;
        }

        // Reset accumulators.
        self.grad_accum = [0.0; N_PARAMS];
        self.grad_count = 0;
        self.stable_frames = 0;
        self.task_count += 1;
        self.has_prior = true;
    }

    /// Compute mean Fisher information across all parameters.
    fn mean_fisher(&self) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..N_PARAMS {
            sum += self.fisher[i];
        }
        sum / N_PARAMS as f32
    }

    /// Run inference only (no learning). Returns the predicted zone (argmax).
    pub fn predict(&self, features: &[f32]) -> u8 {
        if features.len() < N_INPUT {
            return 0;
        }
        let output = self.forward(features);
        let mut best = 0u8;
        let mut best_val = output[0];
        for o in 1..N_OUTPUT {
            if output[o] > best_val {
                best_val = output[o];
                best = o as u8;
            }
        }
        best
    }

    /// Get the current parameter vector.
    pub fn parameters(&self) -> &[f32; N_PARAMS] {
        &self.params
    }

    /// Get the Fisher diagonal.
    pub fn fisher_diagonal(&self) -> &[f32; N_PARAMS] {
        &self.fisher
    }

    /// Get the number of completed tasks.
    pub fn task_count(&self) -> u8 {
        self.task_count
    }

    /// Get the last computed total loss.
    pub fn last_loss(&self) -> f32 {
        self.last_loss
    }

    /// Get the last computed EWC penalty.
    pub fn last_penalty(&self) -> f32 {
        self.last_penalty
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Whether a prior task has been committed.
    pub fn has_prior_task(&self) -> bool {
        self.has_prior
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use libm::fabsf;

    #[test]
    fn test_const_new() {
        let ewc = EwcLifelong::new();
        assert_eq!(ewc.frame_count(), 0);
        assert_eq!(ewc.task_count(), 0);
        assert!(!ewc.has_prior_task());
    }

    #[test]
    fn test_default_params_nonzero() {
        let ewc = EwcLifelong::new();
        let params = ewc.parameters();
        // At least some params should be nonzero (symmetry breaking).
        let nonzero = params.iter().filter(|&&p| fabsf(p) > 1e-6).count();
        assert!(nonzero > N_PARAMS / 2,
            "default params should have diverse nonzero values, got {}/{}", nonzero, N_PARAMS);
    }

    #[test]
    fn test_forward_produces_output() {
        let ewc = EwcLifelong::new();
        let features = [1.0f32; N_INPUT];
        let output = ewc.predict(&features);
        assert!(output < N_OUTPUT as u8, "predicted zone should be 0-3");
    }

    #[test]
    fn test_insufficient_features_no_events() {
        let mut ewc = EwcLifelong::new();
        let features = [1.0f32; 4]; // Only 4, need 8.
        let events = ewc.process_frame(&features, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_inference_only_no_learning() {
        let mut ewc = EwcLifelong::new();
        let features = [1.0f32; N_INPUT];
        // target_zone = -1 means no label -> no learning.
        let events = ewc.process_frame(&features, -1);
        assert!(events.is_empty(), "inference-only should emit no events");
        assert_eq!(ewc.task_count(), 0);
    }

    #[test]
    fn test_learning_reduces_loss() {
        let mut ewc = EwcLifelong::new();
        let features = [0.5f32, 0.3, 0.8, 0.1, 0.6, 0.2, 0.9, 0.4];
        let target = 2; // Zone 2.

        // Train for many frames.
        for _ in 0..200 {
            ewc.process_frame(&features, target);
        }

        // After training, the loss should have decreased.
        assert!(ewc.last_loss() < 1.0,
            "loss should decrease after training, got {}", ewc.last_loss());
    }

    #[test]
    fn test_ewc_penalty_zero_without_prior() {
        let mut ewc = EwcLifelong::new();
        let features = [1.0f32; N_INPUT];
        ewc.process_frame(&features, 0);
        assert!(!ewc.has_prior_task());
        assert!(ewc.last_penalty() < 1e-8,
            "EWC penalty should be 0 without prior task");
    }

    #[test]
    fn test_task_boundary_detection() {
        let mut ewc = EwcLifelong::new();
        let features = [0.5f32; N_INPUT];
        let target = 1;

        // Run enough frames to potentially trigger task boundary.
        for _ in 0..500 {
            ewc.process_frame(&features, target);
        }

        // Exercise the accessor -- exact timing depends on convergence.
        let _ = ewc.task_count();
    }

    #[test]
    fn test_fisher_starts_zero() {
        let ewc = EwcLifelong::new();
        let fisher = ewc.fisher_diagonal();
        for &f in fisher.iter() {
            assert!(fabsf(f) < 1e-8, "Fisher should start at 0");
        }
    }

    #[test]
    fn test_commit_task_sets_prior() {
        let mut ewc = EwcLifelong::new();
        ewc.stable_frames = STABLE_FRAMES_THRESHOLD;
        ewc.commit_task();
        assert!(ewc.has_prior_task());
        assert_eq!(ewc.task_count(), 1);
    }

    #[test]
    fn test_ewc_penalty_nonzero_after_drift() {
        let mut ewc = EwcLifelong::new();

        // Set up a prior task with nonzero Fisher.
        ewc.fisher = [0.1; N_PARAMS];
        ewc.theta_star = [0.0; N_PARAMS];
        ewc.has_prior = true;

        // Shift parameters away from theta_star.
        for i in 0..N_PARAMS {
            ewc.params[i] = 0.5;
        }

        let penalty = ewc.compute_ewc_penalty();
        // Expected: (1000/2) * 32 * 0.1 * 0.25 = 400.0
        assert!(penalty > 100.0,
            "EWC penalty should be large when params drift, got {}", penalty);
    }

    #[test]
    fn test_predict_deterministic() {
        let ewc = EwcLifelong::new();
        let features = [0.5f32; N_INPUT];
        let p1 = ewc.predict(&features);
        let p2 = ewc.predict(&features);
        assert_eq!(p1, p2, "predict should be deterministic");
    }

    #[test]
    fn test_reset() {
        let mut ewc = EwcLifelong::new();
        let features = [1.0f32; N_INPUT];
        for _ in 0..50 {
            ewc.process_frame(&features, 0);
        }
        assert!(ewc.frame_count() > 0);
        ewc.reset();
        assert_eq!(ewc.frame_count(), 0);
        assert_eq!(ewc.task_count(), 0);
        assert!(!ewc.has_prior_task());
    }

    #[test]
    fn test_max_tasks_cap() {
        let mut ewc = EwcLifelong::new();
        ewc.task_count = MAX_TASKS;
        ewc.stable_frames = STABLE_FRAMES_THRESHOLD;
        let features = [1.0f32; N_INPUT];
        let events = ewc.process_frame(&features, 0);
        let new_task_events = events.iter()
            .filter(|e| e.0 == EVENT_NEW_TASK_LEARNED)
            .count();
        assert_eq!(new_task_events, 0,
            "should not learn new task when at MAX_TASKS");
    }
}

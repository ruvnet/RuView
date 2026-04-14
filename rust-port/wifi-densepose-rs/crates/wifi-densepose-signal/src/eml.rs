//! EML (Exp-Ln) Learned Functions Module
//!
//! Implements the EML operator `eml(x, y) = exp(x) - ln(y)` as a universal
//! function approximator, based on:
//!
//!   Odrzywolel 2026, "All elementary functions from a single operator"
//!   (arXiv:2603.21852v2)
//!
//! The EML operator is the continuous-math analog of the NAND gate: a single
//! binary operator that can reconstruct all elementary functions. Combined with
//! gradient-free training (coordinate descent), it discovers closed-form
//! mathematical relationships from data.
//!
//! # Design
//!
//! An `EmlModel` is a binary tree of EML operators with trainable leaf
//! parameters. Each leaf is either an input variable or a learned constant.
//! Training uses coordinate descent (no backprop needed), making it suitable
//! for edge devices.
//!
//! # Usage
//!
//! ```rust,ignore
//! use wifi_densepose_signal::eml::{EmlModel, EmlConfig};
//!
//! let config = EmlConfig { depth: 3, n_inputs: 4, n_outputs: 1 };
//! let mut model = EmlModel::new(config);
//!
//! // Train on (inputs, targets) pairs
//! let inputs = vec![vec![0.5, 0.3, 0.7, 0.1]];
//! let targets = vec![vec![0.6]];
//! model.train(&inputs, &targets, 100);
//!
//! // Predict
//! let output = model.predict(&[0.5, 0.3, 0.7, 0.1]);
//! ```

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for an EML model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmlConfig {
    /// Depth of the binary tree (number of layers).
    pub depth: usize,
    /// Number of input variables.
    pub n_inputs: usize,
    /// Number of output heads.
    pub n_outputs: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Node
// ─────────────────────────────────────────────────────────────────────────────

/// A node in the EML computation tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum EmlNode {
    /// Leaf node: either references an input variable or holds a constant.
    Leaf {
        /// Index into the input vector, or `None` for a learned constant.
        input_idx: Option<usize>,
        /// Learned constant value (used when `input_idx` is `None`,
        /// or as a bias added to the input variable).
        bias: f64,
        /// Scaling factor applied before the node value is used.
        scale: f64,
    },
    /// Internal node: applies `eml(left, right) = exp(left) - ln(right)`.
    Internal {
        left: Box<EmlNode>,
        right: Box<EmlNode>,
        /// Output scaling factor.
        scale: f64,
    },
}

impl EmlNode {
    /// Evaluate this subtree given input values.
    fn evaluate(&self, inputs: &[f64]) -> f64 {
        match self {
            EmlNode::Leaf {
                input_idx,
                bias,
                scale,
            } => {
                let base = input_idx
                    .map(|i| inputs.get(i).copied().unwrap_or(0.0))
                    .unwrap_or(0.0);
                scale * (base + bias)
            }
            EmlNode::Internal { left, right, scale } => {
                let l = left.evaluate(inputs);
                let r = right.evaluate(inputs);
                // eml(x, y) = exp(x) - ln(y)
                // Guard against domain errors: clamp r > 0 for ln.
                let r_safe = r.abs().max(1e-10);
                let result = l.clamp(-10.0, 10.0).exp() - r_safe.ln();
                scale * result
            }
        }
    }

    /// Collect all trainable parameters as mutable references.
    fn collect_params(&mut self) -> Vec<*mut f64> {
        match self {
            EmlNode::Leaf { bias, scale, .. } => {
                vec![bias as *mut f64, scale as *mut f64]
            }
            EmlNode::Internal { left, right, scale, .. } => {
                let mut params = left.collect_params();
                params.extend(right.collect_params());
                params.push(scale as *mut f64);
                params
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Model
// ─────────────────────────────────────────────────────────────────────────────

/// An EML model consisting of one binary tree per output head.
///
/// Each tree has `2^depth - 1` internal nodes and `2^depth` leaf nodes.
/// Total trainable parameters per head: `2 * 2^depth` (leaf bias + scale)
/// plus `2^depth - 1` (internal scales).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmlModel {
    config: EmlConfig,
    /// One tree per output head.
    trees: Vec<EmlNode>,
    /// Whether the model has been trained.
    trained: bool,
}

impl EmlModel {
    /// Create a new EML model with the given configuration.
    ///
    /// Trees are initialized with small random-ish constants that
    /// approximate the identity function when possible.
    pub fn new(config: EmlConfig) -> Self {
        let trees = (0..config.n_outputs)
            .map(|head| Self::build_tree(config.depth, config.n_inputs, head))
            .collect();
        Self {
            config,
            trees,
            trained: false,
        }
    }

    /// Build a balanced binary tree of the given depth.
    fn build_tree(depth: usize, n_inputs: usize, head_idx: usize) -> EmlNode {
        if depth == 0 {
            // Leaf: assign to an input variable round-robin.
            let input_idx = if n_inputs > 0 {
                Some((head_idx) % n_inputs)
            } else {
                None
            };
            EmlNode::Leaf {
                input_idx,
                bias: 0.0,
                scale: 1.0,
            }
        } else {
            let left_input_offset = head_idx * 2;
            let right_input_offset = head_idx * 2 + 1;
            EmlNode::Internal {
                left: Box::new(Self::build_tree(
                    depth - 1,
                    n_inputs,
                    left_input_offset % n_inputs.max(1),
                )),
                right: Box::new(Self::build_tree(
                    depth - 1,
                    n_inputs,
                    right_input_offset % n_inputs.max(1),
                )),
                scale: 0.01, // Small initial scale to keep outputs near zero.
            }
        }
    }

    /// Predict output values for the given inputs.
    ///
    /// Returns a vector of length `n_outputs`.
    pub fn predict(&self, inputs: &[f64]) -> Vec<f64> {
        self.trees.iter().map(|tree| tree.evaluate(inputs)).collect()
    }

    /// Train the model using coordinate descent (gradient-free).
    ///
    /// - `data`: slice of (inputs, targets) where each element is
    ///   `(Vec<f64>, Vec<f64>)`.
    /// - `epochs`: number of coordinate descent passes.
    /// - `step_size`: initial perturbation magnitude.
    ///
    /// Returns the final mean squared error.
    pub fn train(
        &mut self,
        inputs: &[Vec<f64>],
        targets: &[Vec<f64>],
        epochs: usize,
        step_size: f64,
    ) -> f64 {
        if inputs.is_empty() || targets.is_empty() || inputs.len() != targets.len() {
            return f64::MAX;
        }

        let mut best_loss = self.compute_loss(inputs, targets);

        for epoch in 0..epochs {
            let current_step = step_size * (1.0 / (1.0 + epoch as f64 * 0.01));

            for tree_idx in 0..self.trees.len() {
                // Collect parameter pointers for this tree.
                let params = self.trees[tree_idx].collect_params();

                for param_ptr in params {
                    // Safety: we own all the data in `self.trees` and the
                    // pointers point into our own struct fields. No aliasing
                    // occurs because we process one parameter at a time.
                    let original = unsafe { *param_ptr };

                    // Try positive perturbation.
                    unsafe { *param_ptr = original + current_step };
                    let loss_plus = self.compute_loss(inputs, targets);

                    // Try negative perturbation.
                    unsafe { *param_ptr = original - current_step };
                    let loss_minus = self.compute_loss(inputs, targets);

                    // Keep the best.
                    if loss_plus < best_loss && loss_plus <= loss_minus {
                        unsafe { *param_ptr = original + current_step };
                        best_loss = loss_plus;
                    } else if loss_minus < best_loss {
                        unsafe { *param_ptr = original - current_step };
                        best_loss = loss_minus;
                    } else {
                        // Revert.
                        unsafe { *param_ptr = original };
                    }
                }
            }
        }

        self.trained = true;
        best_loss
    }

    /// Compute mean squared error across all samples and outputs.
    fn compute_loss(&self, inputs: &[Vec<f64>], targets: &[Vec<f64>]) -> f64 {
        let mut total = 0.0;
        let mut count = 0;

        for (inp, tgt) in inputs.iter().zip(targets.iter()) {
            let pred = self.predict(inp);
            for (p, t) in pred.iter().zip(tgt.iter()) {
                let diff = p - t;
                // Guard against NaN/Inf from exp overflow.
                if diff.is_finite() {
                    total += diff * diff;
                } else {
                    total += 1e6; // Penalty for non-finite outputs.
                }
                count += 1;
            }
        }

        if count > 0 {
            total / count as f64
        } else {
            f64::MAX
        }
    }

    /// Whether this model has been trained.
    pub fn is_trained(&self) -> bool {
        self.trained
    }

    /// Number of trainable parameters across all output heads.
    pub fn param_count(&self) -> usize {
        self.trees
            .iter()
            .map(|tree| Self::count_params(tree))
            .sum()
    }

    fn count_params(node: &EmlNode) -> usize {
        match node {
            EmlNode::Leaf { .. } => 2, // bias + scale
            EmlNode::Internal { left, right, .. } => {
                1 + Self::count_params(left) + Self::count_params(right)
            }
        }
    }

    /// Serialize the model to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize the model from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eml_model_creation() {
        let config = EmlConfig {
            depth: 3,
            n_inputs: 4,
            n_outputs: 1,
        };
        let model = EmlModel::new(config);
        assert!(!model.is_trained());
        assert!(model.param_count() > 0);
    }

    #[test]
    fn test_eml_predict_shape() {
        let config = EmlConfig {
            depth: 2,
            n_inputs: 3,
            n_outputs: 2,
        };
        let model = EmlModel::new(config);
        let output = model.predict(&[0.5, 0.3, 0.7]);
        assert_eq!(output.len(), 2);
        // Outputs should be finite.
        for v in &output {
            assert!(v.is_finite(), "output should be finite: {v}");
        }
    }

    #[test]
    fn test_eml_train_reduces_loss() {
        let config = EmlConfig {
            depth: 2,
            n_inputs: 2,
            n_outputs: 1,
        };
        let mut model = EmlModel::new(config);

        // Simple target: output = 0.5 for all inputs.
        let inputs: Vec<Vec<f64>> = (0..20)
            .map(|i| vec![i as f64 * 0.05, 1.0 - i as f64 * 0.05])
            .collect();
        let targets: Vec<Vec<f64>> = vec![vec![0.5]; 20];

        let initial_loss = model.compute_loss(&inputs, &targets);
        let final_loss = model.train(&inputs, &targets, 50, 0.1);

        assert!(model.is_trained());
        assert!(
            final_loss <= initial_loss + 1e-6,
            "training should not increase loss: initial={initial_loss}, final={final_loss}"
        );
    }

    #[test]
    fn test_eml_serialization() {
        let config = EmlConfig {
            depth: 2,
            n_inputs: 3,
            n_outputs: 1,
        };
        let model = EmlModel::new(config);
        let json = model.to_json().unwrap();
        let restored = EmlModel::from_json(&json).unwrap();
        assert_eq!(model.param_count(), restored.param_count());

        // Predictions should match.
        let input = vec![0.5, 0.3, 0.7];
        let orig = model.predict(&input);
        let rest = restored.predict(&input);
        for (a, b) in orig.iter().zip(rest.iter()) {
            assert!((a - b).abs() < 1e-10);
        }
    }
}

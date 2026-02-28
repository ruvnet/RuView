//! Sense Layer - Policy Network for Routing Decisions
//!
//! This module implements the policy network that decides, for each retrieved chunk,
//! whether to return the compressed tensor (COMPRESS) or the raw text (EXPAND).
//!
//! The policy is a lightweight classifier that runs in <50 microseconds per decision.

use crate::types::{RefragEntry, RefragResponseType};
use ndarray::{Array1, Array2};
use rand::Rng;
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PolicyError {
    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Invalid policy weights: {0}")]
    InvalidWeights(String),
}

pub type Result<T> = std::result::Result<T, PolicyError>;

/// Action decided by the policy network
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefragAction {
    /// Return compressed tensor representation
    Compress,
    /// Return expanded text content
    Expand,
}

impl From<RefragAction> for RefragResponseType {
    fn from(action: RefragAction) -> Self {
        match action {
            RefragAction::Compress => RefragResponseType::Compress,
            RefragAction::Expand => RefragResponseType::Expand,
        }
    }
}

/// Policy decision with confidence
#[derive(Debug, Clone)]
pub struct PolicyDecision {
    /// Recommended action
    pub action: RefragAction,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Raw logit/score from policy
    pub raw_score: f32,
    /// Decision latency in microseconds
    pub latency_us: u64,
}

/// Trait for policy models
pub trait PolicyModel: Send + Sync {
    /// Decide action for a single chunk
    fn decide(&self, chunk_tensor: &[f32], query_tensor: &[f32]) -> Result<PolicyDecision>;

    /// Batch decision for multiple chunks
    fn decide_batch(&self, chunks: &[&[f32]], query_tensor: &[f32]) -> Result<Vec<PolicyDecision>> {
        chunks
            .iter()
            .map(|chunk| self.decide(chunk, query_tensor))
            .collect()
    }

    /// Get model info
    fn info(&self) -> PolicyModelInfo;
}

/// Policy model metadata
#[derive(Debug, Clone)]
pub struct PolicyModelInfo {
    pub name: String,
    pub input_dim: usize,
    pub version: String,
    pub avg_latency_us: f64,
}

/// Linear policy network (single layer)
///
/// Decision: sigmoid(W @ [chunk; query] + b) > threshold
pub struct LinearPolicy {
    /// Weight matrix [1, input_dim * 2]
    weights: Array1<f32>,
    /// Bias term
    bias: f32,
    /// Decision threshold
    threshold: f32,
    /// Input dimension (for chunk or query)
    input_dim: usize,
}

impl LinearPolicy {
    /// Create a new linear policy with random initialization
    pub fn new(input_dim: usize, threshold: f32) -> Self {
        let mut rng = rand::thread_rng();
        let combined_dim = input_dim * 2;

        // Xavier initialization
        let scale = (2.0 / combined_dim as f32).sqrt();
        let weights: Vec<f32> = (0..combined_dim)
            .map(|_| rng.gen_range(-scale..scale))
            .collect();

        Self {
            weights: Array1::from_vec(weights),
            bias: 0.0,
            threshold,
            input_dim,
        }
    }

    /// Create with specific weights
    pub fn with_weights(weights: Vec<f32>, bias: f32, threshold: f32) -> Result<Self> {
        if weights.is_empty() || weights.len() % 2 != 0 {
            return Err(PolicyError::InvalidWeights(
                "Weights length must be even (chunk_dim + query_dim)".into(),
            ));
        }

        let input_dim = weights.len() / 2;
        Ok(Self {
            weights: Array1::from_vec(weights),
            bias,
            threshold,
            input_dim,
        })
    }

    /// Load weights from a simple binary format
    pub fn load_weights(data: &[u8], threshold: f32) -> Result<Self> {
        if data.len() < 8 {
            return Err(PolicyError::InvalidWeights("Data too short".into()));
        }

        // Format: [input_dim: u32][bias: f32][weights: f32 * dim * 2]
        let input_dim = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let bias = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        let expected_len = 8 + input_dim * 2 * 4;
        if data.len() != expected_len {
            return Err(PolicyError::InvalidWeights(format!(
                "Expected {} bytes, got {}",
                expected_len,
                data.len()
            )));
        }

        let mut weights = Vec::with_capacity(input_dim * 2);
        for chunk in data[8..].chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            weights.push(f32::from_le_bytes(bytes));
        }

        Self::with_weights(weights, bias, threshold)
    }

    /// Export weights to binary format
    pub fn export_weights(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + self.weights.len() * 4);

        data.extend_from_slice(&(self.input_dim as u32).to_le_bytes());
        data.extend_from_slice(&self.bias.to_le_bytes());

        for &w in self.weights.iter() {
            data.extend_from_slice(&w.to_le_bytes());
        }

        data
    }

    /// Sigmoid activation
    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }
}

impl PolicyModel for LinearPolicy {
    fn decide(&self, chunk_tensor: &[f32], query_tensor: &[f32]) -> Result<PolicyDecision> {
        let start = Instant::now();

        if chunk_tensor.len() != self.input_dim {
            return Err(PolicyError::DimensionMismatch {
                expected: self.input_dim,
                actual: chunk_tensor.len(),
            });
        }
        if query_tensor.len() != self.input_dim {
            return Err(PolicyError::DimensionMismatch {
                expected: self.input_dim,
                actual: query_tensor.len(),
            });
        }

        // Concatenate chunk and query
        let mut combined = Vec::with_capacity(self.input_dim * 2);
        combined.extend_from_slice(chunk_tensor);
        combined.extend_from_slice(query_tensor);

        // Dot product with weights
        let logit: f32 = combined
            .iter()
            .zip(self.weights.iter())
            .map(|(x, w)| x * w)
            .sum::<f32>()
            + self.bias;

        let score = Self::sigmoid(logit);
        let action = if score > self.threshold {
            RefragAction::Compress
        } else {
            RefragAction::Expand
        };

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(PolicyDecision {
            action,
            confidence: if action == RefragAction::Compress {
                score
            } else {
                1.0 - score
            },
            raw_score: score,
            latency_us,
        })
    }

    fn info(&self) -> PolicyModelInfo {
        PolicyModelInfo {
            name: "LinearPolicy".to_string(),
            input_dim: self.input_dim,
            version: "1.0.0".to_string(),
            avg_latency_us: 5.0, // Typical for simple dot product
        }
    }
}

/// MLP Policy Network (two hidden layers)
pub struct MLPPolicy {
    /// First layer weights [hidden_dim, input_dim * 2]
    w1: Array2<f32>,
    /// First layer bias
    b1: Array1<f32>,
    /// Second layer weights [1, hidden_dim]
    w2: Array1<f32>,
    /// Second layer bias
    b2: f32,
    /// Decision threshold
    threshold: f32,
    /// Input dimension
    input_dim: usize,
    /// Hidden dimension
    hidden_dim: usize,
}

impl MLPPolicy {
    /// Create a new MLP policy with random initialization
    pub fn new(input_dim: usize, hidden_dim: usize, threshold: f32) -> Self {
        let mut rng = rand::thread_rng();
        let combined_dim = input_dim * 2;

        // Xavier initialization for first layer
        let scale1 = (2.0 / combined_dim as f32).sqrt();
        let w1_data: Vec<f32> = (0..hidden_dim * combined_dim)
            .map(|_| rng.gen_range(-scale1..scale1))
            .collect();

        // Xavier initialization for second layer
        let scale2 = (2.0 / hidden_dim as f32).sqrt();
        let w2_data: Vec<f32> = (0..hidden_dim)
            .map(|_| rng.gen_range(-scale2..scale2))
            .collect();

        Self {
            w1: Array2::from_shape_vec((hidden_dim, combined_dim), w1_data).unwrap(),
            b1: Array1::zeros(hidden_dim),
            w2: Array1::from_vec(w2_data),
            b2: 0.0,
            threshold,
            input_dim,
            hidden_dim,
        }
    }

    /// ReLU activation
    fn relu(x: f32) -> f32 {
        x.max(0.0)
    }

    /// Sigmoid activation
    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }
}

impl PolicyModel for MLPPolicy {
    fn decide(&self, chunk_tensor: &[f32], query_tensor: &[f32]) -> Result<PolicyDecision> {
        let start = Instant::now();

        if chunk_tensor.len() != self.input_dim {
            return Err(PolicyError::DimensionMismatch {
                expected: self.input_dim,
                actual: chunk_tensor.len(),
            });
        }
        if query_tensor.len() != self.input_dim {
            return Err(PolicyError::DimensionMismatch {
                expected: self.input_dim,
                actual: query_tensor.len(),
            });
        }

        // Concatenate inputs
        let mut combined = Vec::with_capacity(self.input_dim * 2);
        combined.extend_from_slice(chunk_tensor);
        combined.extend_from_slice(query_tensor);
        let input = Array1::from_vec(combined);

        // First layer: h = ReLU(W1 @ x + b1)
        let mut hidden = Array1::zeros(self.hidden_dim);
        for i in 0..self.hidden_dim {
            let dot: f32 = self
                .w1
                .row(i)
                .iter()
                .zip(input.iter())
                .map(|(w, x)| w * x)
                .sum();
            hidden[i] = Self::relu(dot + self.b1[i]);
        }

        // Second layer: logit = W2 @ h + b2
        let logit: f32 = self
            .w2
            .iter()
            .zip(hidden.iter())
            .map(|(w, h)| w * h)
            .sum::<f32>()
            + self.b2;

        let score = Self::sigmoid(logit);
        let action = if score > self.threshold {
            RefragAction::Compress
        } else {
            RefragAction::Expand
        };

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(PolicyDecision {
            action,
            confidence: if action == RefragAction::Compress {
                score
            } else {
                1.0 - score
            },
            raw_score: score,
            latency_us,
        })
    }

    fn info(&self) -> PolicyModelInfo {
        PolicyModelInfo {
            name: "MLPPolicy".to_string(),
            input_dim: self.input_dim,
            version: "1.0.0".to_string(),
            avg_latency_us: 15.0, // Typical for small MLP
        }
    }
}

/// Simple threshold-based policy (no learned weights)
pub struct ThresholdPolicy {
    /// Similarity threshold
    threshold: f32,
}

impl ThresholdPolicy {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

impl PolicyModel for ThresholdPolicy {
    fn decide(&self, chunk_tensor: &[f32], query_tensor: &[f32]) -> Result<PolicyDecision> {
        let start = Instant::now();

        let similarity = Self::cosine_similarity(chunk_tensor, query_tensor);

        // High similarity = COMPRESS (tensor is good representation)
        // Low similarity = EXPAND (need full text for context)
        let action = if similarity > self.threshold {
            RefragAction::Compress
        } else {
            RefragAction::Expand
        };

        let latency_us = start.elapsed().as_micros() as u64;

        Ok(PolicyDecision {
            action,
            confidence: similarity.abs(),
            raw_score: similarity,
            latency_us,
        })
    }

    fn info(&self) -> PolicyModelInfo {
        PolicyModelInfo {
            name: "ThresholdPolicy".to_string(),
            input_dim: 0, // Any dimension
            version: "1.0.0".to_string(),
            avg_latency_us: 2.0, // Just cosine similarity
        }
    }
}

/// Policy network wrapper with caching
pub struct PolicyNetwork {
    policy: Box<dyn PolicyModel>,
    /// Cache recent decisions
    cache_enabled: bool,
}

impl PolicyNetwork {
    pub fn new(policy: Box<dyn PolicyModel>) -> Self {
        Self {
            policy,
            cache_enabled: false,
        }
    }

    pub fn linear(input_dim: usize, threshold: f32) -> Self {
        Self::new(Box::new(LinearPolicy::new(input_dim, threshold)))
    }

    pub fn mlp(input_dim: usize, hidden_dim: usize, threshold: f32) -> Self {
        Self::new(Box::new(MLPPolicy::new(input_dim, hidden_dim, threshold)))
    }

    pub fn threshold(threshold: f32) -> Self {
        Self::new(Box::new(ThresholdPolicy::new(threshold)))
    }

    pub fn with_caching(mut self, enabled: bool) -> Self {
        self.cache_enabled = enabled;
        self
    }

    pub fn decide(&self, chunk_tensor: &[f32], query_tensor: &[f32]) -> Result<PolicyDecision> {
        self.policy.decide(chunk_tensor, query_tensor)
    }

    pub fn decide_batch(
        &self,
        chunks: &[&[f32]],
        query_tensor: &[f32],
    ) -> Result<Vec<PolicyDecision>> {
        self.policy.decide_batch(chunks, query_tensor)
    }

    pub fn info(&self) -> PolicyModelInfo {
        self.policy.info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_policy() {
        let policy = LinearPolicy::new(4, 0.5);

        let chunk = vec![0.1, 0.2, 0.3, 0.4];
        let query = vec![0.4, 0.3, 0.2, 0.1];

        let decision = policy.decide(&chunk, &query).unwrap();
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        assert!(decision.latency_us < 1000); // Should be < 1ms
    }

    #[test]
    fn test_mlp_policy() {
        let policy = MLPPolicy::new(4, 8, 0.5);

        let chunk = vec![0.1, 0.2, 0.3, 0.4];
        let query = vec![0.4, 0.3, 0.2, 0.1];

        let decision = policy.decide(&chunk, &query).unwrap();
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        assert!(decision.latency_us < 1000); // Should be < 1ms
    }

    #[test]
    fn test_threshold_policy() {
        let policy = ThresholdPolicy::new(0.9);

        // Similar vectors -> COMPRESS
        let chunk = vec![1.0, 0.0, 0.0, 0.0];
        let query = vec![0.99, 0.01, 0.0, 0.0];
        let decision = policy.decide(&chunk, &query).unwrap();
        assert_eq!(decision.action, RefragAction::Compress);

        // Different vectors -> EXPAND
        let chunk = vec![1.0, 0.0, 0.0, 0.0];
        let query = vec![0.0, 1.0, 0.0, 0.0];
        let decision = policy.decide(&chunk, &query).unwrap();
        assert_eq!(decision.action, RefragAction::Expand);
    }

    #[test]
    fn test_policy_network_wrapper() {
        let network = PolicyNetwork::threshold(0.5);

        let chunk = vec![0.5, 0.5, 0.5, 0.5];
        let query = vec![0.5, 0.5, 0.5, 0.5];

        let decision = network.decide(&chunk, &query).unwrap();
        assert_eq!(decision.action, RefragAction::Compress); // Identical vectors

        let info = network.info();
        assert_eq!(info.name, "ThresholdPolicy");
    }

    #[test]
    fn test_dimension_mismatch() {
        let policy = LinearPolicy::new(4, 0.5);

        let chunk = vec![0.1, 0.2, 0.3]; // Wrong size
        let query = vec![0.4, 0.3, 0.2, 0.1];

        let result = policy.decide(&chunk, &query);
        assert!(matches!(result, Err(PolicyError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_weight_export_import() {
        let policy = LinearPolicy::new(4, 0.7);
        let exported = policy.export_weights();

        let imported = LinearPolicy::load_weights(&exported, 0.7).unwrap();

        // Verify same behavior
        let chunk = vec![0.1, 0.2, 0.3, 0.4];
        let query = vec![0.4, 0.3, 0.2, 0.1];

        let d1 = policy.decide(&chunk, &query).unwrap();
        let d2 = imported.decide(&chunk, &query).unwrap();

        assert_eq!(d1.action, d2.action);
        assert!((d1.raw_score - d2.raw_score).abs() < f32::EPSILON);
    }
}

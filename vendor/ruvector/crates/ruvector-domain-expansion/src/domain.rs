//! Core domain trait and types for cross-domain transfer learning.
//!
//! A domain defines a problem space with:
//! - A task generator (produces training instances)
//! - An evaluator (scores solutions on [0.0, 1.0])
//! - Embedding extraction (maps solutions into a shared representation space)
//!
//! True IQ growth appears when a kernel trained on Domain 1 improves Domain 2
//! faster than Domain 2 alone. That is generalization.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a domain.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainId(pub String);

impl fmt::Display for DomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single task instance within a domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: String,
    /// Domain this task belongs to.
    pub domain_id: DomainId,
    /// Difficulty level [0.0, 1.0].
    pub difficulty: f32,
    /// Structured task specification (domain-specific JSON).
    pub spec: serde_json::Value,
    /// Optional constraints the solution must satisfy.
    pub constraints: Vec<String>,
}

/// A candidate solution to a domain task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    /// The task this solves.
    pub task_id: String,
    /// Raw solution content (e.g., Rust source, plan steps, tool calls).
    pub content: String,
    /// Structured solution data (domain-specific).
    pub data: serde_json::Value,
}

/// Evaluation result for a solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    /// Overall score [0.0, 1.0] where 1.0 is perfect.
    pub score: f32,
    /// Correctness: does it produce the right answer?
    pub correctness: f32,
    /// Efficiency: resource usage relative to optimal.
    pub efficiency: f32,
    /// Elegance: structural quality, idiomatic patterns.
    pub elegance: f32,
    /// Per-constraint pass/fail results.
    pub constraint_results: Vec<bool>,
    /// Diagnostic notes from the evaluator.
    pub notes: Vec<String>,
}

impl Evaluation {
    /// Create a zero-score evaluation (failure).
    pub fn zero(notes: Vec<String>) -> Self {
        Self {
            score: 0.0,
            correctness: 0.0,
            efficiency: 0.0,
            elegance: 0.0,
            constraint_results: Vec::new(),
            notes,
        }
    }

    /// Compute composite score from weighted sub-scores.
    pub fn composite(correctness: f32, efficiency: f32, elegance: f32) -> Self {
        let score = 0.6 * correctness + 0.25 * efficiency + 0.15 * elegance;
        Self {
            score: score.clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// Embedding vector for cross-domain representation.
/// Solutions from different domains are projected into a shared space
/// so that transfer learning can identify structural similarities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEmbedding {
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Which domain produced this embedding.
    pub domain_id: DomainId,
    /// Dimensionality.
    pub dim: usize,
}

impl DomainEmbedding {
    /// Create a new embedding.
    pub fn new(vector: Vec<f32>, domain_id: DomainId) -> Self {
        let dim = vector.len();
        Self {
            vector,
            domain_id,
            dim,
        }
    }

    /// Cosine similarity with another embedding.
    pub fn cosine_similarity(&self, other: &DomainEmbedding) -> f32 {
        assert_eq!(self.dim, other.dim, "Embedding dimensions must match");

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..self.dim {
            dot += self.vector[i] * other.vector[i];
            norm_a += self.vector[i] * self.vector[i];
            norm_b += other.vector[i] * other.vector[i];
        }

        let denom = (norm_a.sqrt() * norm_b.sqrt()).max(1e-10);
        dot / denom
    }
}

/// Core trait that every domain must implement.
///
/// Domains are problem spaces: Rust program synthesis, structured planning,
/// tool orchestration, etc. Each domain knows how to generate tasks,
/// evaluate solutions, and embed solutions into a shared representation space.
pub trait Domain: Send + Sync {
    /// Unique identifier for this domain.
    fn id(&self) -> &DomainId;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Generate a batch of tasks at the given difficulty level.
    ///
    /// # Arguments
    /// * `count` - Number of tasks to generate
    /// * `difficulty` - Target difficulty [0.0, 1.0]
    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task>;

    /// Evaluate a solution against its task.
    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation;

    /// Project a solution into the shared embedding space.
    /// This enables cross-domain transfer by finding structural similarities
    /// between solutions across different problem domains.
    fn embed(&self, solution: &Solution) -> DomainEmbedding;

    /// Embedding dimensionality for this domain.
    fn embedding_dim(&self) -> usize;

    /// Generate a reference (optimal or near-optimal) solution for a task.
    /// Used for computing efficiency ratios and as training signal.
    fn reference_solution(&self, task: &Task) -> Option<Solution>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_id_display() {
        let id = DomainId("rust_synthesis".to_string());
        assert_eq!(format!("{}", id), "rust_synthesis");
    }

    #[test]
    fn test_evaluation_zero() {
        let eval = Evaluation::zero(vec!["compile error".to_string()]);
        assert_eq!(eval.score, 0.0);
        assert_eq!(eval.notes.len(), 1);
    }

    #[test]
    fn test_evaluation_composite() {
        let eval = Evaluation::composite(1.0, 0.8, 0.6);
        // 0.6*1.0 + 0.25*0.8 + 0.15*0.6 = 0.6 + 0.2 + 0.09 = 0.89
        assert!((eval.score - 0.89).abs() < 1e-4);
    }

    #[test]
    fn test_embedding_cosine_similarity() {
        let id = DomainId("test".to_string());
        let a = DomainEmbedding::new(vec![1.0, 0.0, 0.0], id.clone());
        let b = DomainEmbedding::new(vec![1.0, 0.0, 0.0], id.clone());
        assert!((a.cosine_similarity(&b) - 1.0).abs() < 1e-6);

        let c = DomainEmbedding::new(vec![0.0, 1.0, 0.0], id);
        assert!(a.cosine_similarity(&c).abs() < 1e-6);
    }

    #[test]
    fn test_evaluation_clamp() {
        let eval = Evaluation::composite(1.0, 1.0, 1.0);
        assert!(eval.score <= 1.0);
    }
}

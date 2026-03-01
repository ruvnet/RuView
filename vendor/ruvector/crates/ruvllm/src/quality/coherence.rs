//! Semantic Coherence Validation
//!
//! This module provides tools for validating semantic consistency,
//! detecting contradictions, and checking logical flow in generated content.

use crate::error::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for coherence validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceConfig {
    /// Minimum similarity threshold for semantic consistency (0.0-1.0)
    pub similarity_threshold: f32,
    /// Maximum allowed contradiction score (0.0-1.0)
    pub contradiction_threshold: f32,
    /// Minimum logical flow score (0.0-1.0)
    pub logical_flow_threshold: f32,
    /// Embedding dimension for semantic comparisons
    pub embedding_dim: usize,
    /// Enable caching of computed embeddings
    pub enable_caching: bool,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Use approximate similarity (faster but less accurate)
    pub use_approximate: bool,
}

impl Default for CoherenceConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            contradiction_threshold: 0.3,
            logical_flow_threshold: 0.6,
            embedding_dim: 768,
            enable_caching: true,
            max_cache_size: 1000,
            use_approximate: false,
        }
    }
}

/// Result of semantic consistency validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticConsistencyResult {
    /// Whether the content is semantically consistent
    pub is_consistent: bool,
    /// Overall consistency score (0.0-1.0)
    pub consistency_score: f32,
    /// Pairwise similarity scores between segments
    pub segment_similarities: Vec<(usize, usize, f32)>,
    /// Segments that are semantically inconsistent
    pub inconsistent_segments: Vec<usize>,
    /// Average similarity across all segment pairs
    pub average_similarity: f32,
    /// Standard deviation of similarities
    pub similarity_std_dev: f32,
}

impl Default for SemanticConsistencyResult {
    fn default() -> Self {
        Self {
            is_consistent: true,
            consistency_score: 1.0,
            segment_similarities: Vec::new(),
            inconsistent_segments: Vec::new(),
            average_similarity: 1.0,
            similarity_std_dev: 0.0,
        }
    }
}

/// Result of contradiction detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionResult {
    /// Whether contradictions were detected
    pub has_contradictions: bool,
    /// Number of contradictions found
    pub contradiction_count: usize,
    /// Specific contradictions with details
    pub contradictions: Vec<Contradiction>,
    /// Overall contradiction score (0.0 = no contradictions, 1.0 = severe)
    pub contradiction_score: f32,
}

impl Default for ContradictionResult {
    fn default() -> Self {
        Self {
            has_contradictions: false,
            contradiction_count: 0,
            contradictions: Vec::new(),
            contradiction_score: 0.0,
        }
    }
}

/// A specific contradiction found in content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    /// First statement/segment index
    pub segment_a: usize,
    /// Second statement/segment index
    pub segment_b: usize,
    /// Text of first segment
    pub text_a: String,
    /// Text of second segment
    pub text_b: String,
    /// Contradiction severity (0.0-1.0)
    pub severity: f32,
    /// Type of contradiction
    pub contradiction_type: ContradictionType,
    /// Human-readable explanation
    pub explanation: String,
}

/// Types of contradictions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContradictionType {
    /// Direct logical contradiction
    Logical,
    /// Temporal inconsistency
    Temporal,
    /// Numeric inconsistency
    Numeric,
    /// Entity attribute mismatch
    AttributeMismatch,
    /// Causal contradiction
    Causal,
    /// Contextual inconsistency
    Contextual,
}

/// Result of logical flow analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalFlowResult {
    /// Whether logical flow is maintained
    pub has_logical_flow: bool,
    /// Overall flow score (0.0-1.0)
    pub flow_score: f32,
    /// Flow violations with details
    pub violations: Vec<CoherenceViolation>,
    /// Transition scores between segments
    pub transition_scores: Vec<f32>,
    /// Suggested improvements
    pub suggestions: Vec<String>,
}

impl Default for LogicalFlowResult {
    fn default() -> Self {
        Self {
            has_logical_flow: true,
            flow_score: 1.0,
            violations: Vec::new(),
            transition_scores: Vec::new(),
            suggestions: Vec::new(),
        }
    }
}

/// A coherence violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceViolation {
    /// Segment index where violation occurs
    pub segment_index: usize,
    /// Type of violation
    pub violation_type: ViolationType,
    /// Severity (0.0-1.0)
    pub severity: f32,
    /// Description of the violation
    pub description: String,
    /// Suggested fix
    pub suggestion: Option<String>,
}

/// Types of coherence violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    /// Abrupt topic change
    TopicShift,
    /// Missing transition
    MissingTransition,
    /// Broken reference
    BrokenReference,
    /// Illogical sequence
    IllogicalSequence,
    /// Incomplete thought
    IncompleteThought,
    /// Non-sequitur
    NonSequitur,
}

/// Semantic coherence validator
pub struct CoherenceValidator {
    /// Configuration
    config: CoherenceConfig,
    /// Embedding cache
    embedding_cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    /// Negation patterns for contradiction detection
    negation_patterns: Vec<String>,
    /// Transition markers for flow analysis
    transition_markers: Vec<String>,
}

impl CoherenceValidator {
    /// Create a new coherence validator
    pub fn new(config: CoherenceConfig) -> Self {
        Self {
            config,
            embedding_cache: Arc::new(RwLock::new(HashMap::new())),
            negation_patterns: vec![
                "not".to_string(),
                "never".to_string(),
                "no".to_string(),
                "none".to_string(),
                "neither".to_string(),
                "nothing".to_string(),
                "without".to_string(),
                "isn't".to_string(),
                "aren't".to_string(),
                "wasn't".to_string(),
                "weren't".to_string(),
                "don't".to_string(),
                "doesn't".to_string(),
                "didn't".to_string(),
                "won't".to_string(),
                "wouldn't".to_string(),
                "couldn't".to_string(),
                "shouldn't".to_string(),
            ],
            transition_markers: vec![
                "however".to_string(),
                "therefore".to_string(),
                "furthermore".to_string(),
                "moreover".to_string(),
                "consequently".to_string(),
                "thus".to_string(),
                "hence".to_string(),
                "additionally".to_string(),
                "nonetheless".to_string(),
                "meanwhile".to_string(),
                "finally".to_string(),
                "first".to_string(),
                "second".to_string(),
                "then".to_string(),
                "next".to_string(),
            ],
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(CoherenceConfig::default())
    }

    /// Validate semantic consistency across content segments
    pub fn validate_semantic_consistency(
        &self,
        segments: &[String],
        embeddings: Option<&[Vec<f32>]>,
    ) -> Result<SemanticConsistencyResult> {
        if segments.is_empty() {
            return Ok(SemanticConsistencyResult::default());
        }

        if segments.len() == 1 {
            return Ok(SemanticConsistencyResult {
                is_consistent: true,
                consistency_score: 1.0,
                ..Default::default()
            });
        }

        // Get or compute embeddings
        let computed_embeddings = match embeddings {
            Some(emb) => emb.to_vec(),
            None => segments
                .iter()
                .map(|s| self.compute_simple_embedding(s))
                .collect(),
        };

        // Compute pairwise similarities
        let mut similarities = Vec::new();
        let mut inconsistent = Vec::new();

        for i in 0..computed_embeddings.len() {
            for j in (i + 1)..computed_embeddings.len() {
                let sim = cosine_similarity(&computed_embeddings[i], &computed_embeddings[j]);
                similarities.push((i, j, sim));

                if sim < self.config.similarity_threshold {
                    if !inconsistent.contains(&i) {
                        inconsistent.push(i);
                    }
                    if !inconsistent.contains(&j) {
                        inconsistent.push(j);
                    }
                }
            }
        }

        // Compute statistics
        let all_sims: Vec<f32> = similarities.iter().map(|(_, _, s)| *s).collect();
        let avg = if all_sims.is_empty() {
            1.0
        } else {
            all_sims.iter().sum::<f32>() / all_sims.len() as f32
        };
        let std_dev = compute_std_dev(&all_sims, avg);

        let consistency_score = avg;
        let is_consistent =
            inconsistent.is_empty() && consistency_score >= self.config.similarity_threshold;

        Ok(SemanticConsistencyResult {
            is_consistent,
            consistency_score,
            segment_similarities: similarities,
            inconsistent_segments: inconsistent,
            average_similarity: avg,
            similarity_std_dev: std_dev,
        })
    }

    /// Detect contradictions in content
    pub fn detect_contradictions(
        &self,
        segments: &[String],
        embeddings: Option<&[Vec<f32>]>,
    ) -> Result<ContradictionResult> {
        if segments.len() < 2 {
            return Ok(ContradictionResult::default());
        }

        let mut contradictions = Vec::new();

        // Check for negation-based contradictions
        for i in 0..segments.len() {
            for j in (i + 1)..segments.len() {
                if let Some(contradiction) =
                    self.check_negation_contradiction(i, j, &segments[i], &segments[j])
                {
                    contradictions.push(contradiction);
                }
            }
        }

        // Check for numeric contradictions
        for i in 0..segments.len() {
            for j in (i + 1)..segments.len() {
                if let Some(contradiction) =
                    self.check_numeric_contradiction(i, j, &segments[i], &segments[j])
                {
                    contradictions.push(contradiction);
                }
            }
        }

        // If embeddings provided, check for semantic contradictions
        if let Some(emb) = embeddings {
            for i in 0..segments.len() {
                for j in (i + 1)..segments.len() {
                    // Very low similarity with negation might indicate contradiction
                    let sim = cosine_similarity(&emb[i], &emb[j]);
                    let has_negation_i = self.contains_negation(&segments[i]);
                    let has_negation_j = self.contains_negation(&segments[j]);

                    if sim < 0.3 && (has_negation_i != has_negation_j) {
                        contradictions.push(Contradiction {
                            segment_a: i,
                            segment_b: j,
                            text_a: segments[i].clone(),
                            text_b: segments[j].clone(),
                            severity: 1.0 - sim,
                            contradiction_type: ContradictionType::Logical,
                            explanation: "Semantic analysis suggests contradiction".to_string(),
                        });
                    }
                }
            }
        }

        let has_contradictions = !contradictions.is_empty();
        let contradiction_count = contradictions.len();
        let max_pairs = segments.len() * (segments.len() - 1) / 2;
        let contradiction_score = if max_pairs > 0 {
            (contradiction_count as f32 / max_pairs as f32).min(1.0)
        } else {
            0.0
        };

        Ok(ContradictionResult {
            has_contradictions,
            contradiction_count,
            contradictions,
            contradiction_score,
        })
    }

    /// Check logical flow between segments
    pub fn check_logical_flow(
        &self,
        segments: &[String],
        embeddings: Option<&[Vec<f32>]>,
    ) -> Result<LogicalFlowResult> {
        if segments.len() < 2 {
            return Ok(LogicalFlowResult::default());
        }

        let mut violations = Vec::new();
        let mut transition_scores = Vec::new();
        let mut suggestions = Vec::new();

        // Get or compute embeddings
        let computed_embeddings = match embeddings {
            Some(emb) => emb.to_vec(),
            None => segments
                .iter()
                .map(|s| self.compute_simple_embedding(s))
                .collect(),
        };

        // Check transitions between consecutive segments
        for i in 0..(segments.len() - 1) {
            let sim = cosine_similarity(&computed_embeddings[i], &computed_embeddings[i + 1]);
            transition_scores.push(sim);

            // Check for abrupt topic shifts
            if sim < 0.4 {
                violations.push(CoherenceViolation {
                    segment_index: i + 1,
                    violation_type: ViolationType::TopicShift,
                    severity: 1.0 - sim,
                    description: format!("Abrupt topic shift between segments {} and {}", i, i + 1),
                    suggestion: Some("Add a transition sentence".to_string()),
                });
                suggestions.push(format!(
                    "Consider adding a transition between segments {} and {}",
                    i,
                    i + 1
                ));
            }

            // Check for missing transitions on medium similarity
            if sim >= 0.4 && sim < 0.6 {
                let has_transition = self.has_transition_marker(&segments[i + 1]);
                if !has_transition {
                    violations.push(CoherenceViolation {
                        segment_index: i + 1,
                        violation_type: ViolationType::MissingTransition,
                        severity: 0.3,
                        description: format!("Missing transition marker at segment {}", i + 1),
                        suggestion: Some("Add a transition word".to_string()),
                    });
                }
            }
        }

        // Calculate overall flow score
        let avg_transition = if transition_scores.is_empty() {
            1.0
        } else {
            transition_scores.iter().sum::<f32>() / transition_scores.len() as f32
        };

        let violation_penalty =
            violations.iter().map(|v| v.severity).sum::<f32>() / segments.len() as f32;

        let flow_score = (avg_transition - violation_penalty * 0.5).clamp(0.0, 1.0);
        let has_logical_flow = flow_score >= self.config.logical_flow_threshold;

        Ok(LogicalFlowResult {
            has_logical_flow,
            flow_score,
            violations,
            transition_scores,
            suggestions,
        })
    }

    /// Compute a simple embedding for a text segment (bag of words style)
    fn compute_simple_embedding(&self, text: &str) -> Vec<f32> {
        // Check cache first
        if self.config.enable_caching {
            let cache = self.embedding_cache.read();
            if let Some(embedding) = cache.get(text) {
                return embedding.clone();
            }
        }

        // Simple character-based embedding (placeholder for actual embedding model)
        let mut embedding = vec![0.0f32; self.config.embedding_dim];
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();

        // Simple hash-based feature extraction
        for (i, word) in words.iter().enumerate() {
            for (j, c) in word.chars().enumerate() {
                let idx =
                    ((c as usize * 31 + j * 17 + i * 13) % self.config.embedding_dim) as usize;
                embedding[idx] += 1.0;
            }
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        // Cache result
        if self.config.enable_caching {
            let mut cache = self.embedding_cache.write();
            if cache.len() < self.config.max_cache_size {
                cache.insert(text.to_string(), embedding.clone());
            }
        }

        embedding
    }

    /// Check if text contains negation words
    fn contains_negation(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        self.negation_patterns
            .iter()
            .any(|pattern| text_lower.contains(pattern))
    }

    /// Check for negation-based contradictions
    fn check_negation_contradiction(
        &self,
        idx_a: usize,
        idx_b: usize,
        text_a: &str,
        text_b: &str,
    ) -> Option<Contradiction> {
        let text_a_lower = text_a.to_lowercase();
        let text_b_lower = text_b.to_lowercase();
        let words_a: Vec<&str> = text_a_lower.split_whitespace().collect();
        let words_b: Vec<&str> = text_b_lower.split_whitespace().collect();

        // Check if one is negation of the other
        let has_neg_a = self.contains_negation(text_a);
        let has_neg_b = self.contains_negation(text_b);

        if has_neg_a != has_neg_b {
            // Check for common content words
            let content_a: Vec<&str> = words_a
                .iter()
                .filter(|w| w.len() > 3 && !self.negation_patterns.contains(&w.to_string()))
                .copied()
                .collect();
            let content_b: Vec<&str> = words_b
                .iter()
                .filter(|w| w.len() > 3 && !self.negation_patterns.contains(&w.to_string()))
                .copied()
                .collect();

            let common: Vec<&str> = content_a
                .iter()
                .filter(|w| content_b.contains(w))
                .copied()
                .collect();

            if common.len() >= 2 {
                return Some(Contradiction {
                    segment_a: idx_a,
                    segment_b: idx_b,
                    text_a: text_a.to_string(),
                    text_b: text_b.to_string(),
                    severity: 0.7,
                    contradiction_type: ContradictionType::Logical,
                    explanation: format!(
                        "Possible negation contradiction on topics: {}",
                        common.join(", ")
                    ),
                });
            }
        }

        None
    }

    /// Check for numeric contradictions
    fn check_numeric_contradiction(
        &self,
        idx_a: usize,
        idx_b: usize,
        text_a: &str,
        text_b: &str,
    ) -> Option<Contradiction> {
        // Extract numbers from both texts
        let numbers_a: Vec<f64> = extract_numbers(text_a);
        let numbers_b: Vec<f64> = extract_numbers(text_b);

        // Simple check: if texts are similar but have different numbers
        if numbers_a.len() == 1 && numbers_b.len() == 1 {
            let num_a = numbers_a[0];
            let num_b = numbers_b[0];

            // Check if numbers are significantly different
            let diff = (num_a - num_b).abs();
            let max_val = num_a.abs().max(num_b.abs());

            if max_val > 0.0 && diff / max_val > 0.5 {
                // Check if surrounding context is similar
                let text_a_no_num = text_a
                    .chars()
                    .filter(|c| !c.is_numeric() && *c != '.')
                    .collect::<String>();
                let text_b_no_num = text_b
                    .chars()
                    .filter(|c| !c.is_numeric() && *c != '.')
                    .collect::<String>();

                let jaccard = jaccard_similarity(&text_a_no_num, &text_b_no_num);

                if jaccard > 0.5 {
                    return Some(Contradiction {
                        segment_a: idx_a,
                        segment_b: idx_b,
                        text_a: text_a.to_string(),
                        text_b: text_b.to_string(),
                        severity: 0.6,
                        contradiction_type: ContradictionType::Numeric,
                        explanation: format!("Numeric inconsistency: {} vs {}", num_a, num_b),
                    });
                }
            }
        }

        None
    }

    /// Check if text has a transition marker
    fn has_transition_marker(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        self.transition_markers
            .iter()
            .any(|marker| text_lower.contains(marker))
    }

    /// Clear the embedding cache
    pub fn clear_cache(&self) {
        let mut cache = self.embedding_cache.write();
        cache.clear();
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Compute standard deviation
fn compute_std_dev(values: &[f32], mean: f32) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }

    let variance: f32 =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (values.len() - 1) as f32;

    variance.sqrt()
}

/// Extract numbers from text
fn extract_numbers(text: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        if c.is_numeric() || c == '.' || (c == '-' && current.is_empty()) {
            current.push(c);
        } else if !current.is_empty() {
            if let Ok(num) = current.parse::<f64>() {
                numbers.push(num);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(num) = current.parse::<f64>() {
            numbers.push(num);
        }
    }

    numbers
}

/// Compute Jaccard similarity between two strings (word-level)
fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_consistency_single_segment() {
        let validator = CoherenceValidator::default_config();
        let segments = vec!["This is a test.".to_string()];

        let result = validator
            .validate_semantic_consistency(&segments, None)
            .unwrap();
        assert!(result.is_consistent);
        assert_eq!(result.consistency_score, 1.0);
    }

    #[test]
    fn test_semantic_consistency_similar_segments() {
        let validator = CoherenceValidator::default_config();
        let segments = vec![
            "The cat sat on the mat.".to_string(),
            "The cat was sitting on the mat.".to_string(),
        ];

        let result = validator
            .validate_semantic_consistency(&segments, None)
            .unwrap();
        assert!(result.consistency_score > 0.5);
    }

    #[test]
    fn test_contradiction_detection_negation() {
        let validator = CoherenceValidator::default_config();
        let segments = vec![
            "The system is running properly.".to_string(),
            "The system is not running properly.".to_string(),
        ];

        let result = validator.detect_contradictions(&segments, None).unwrap();
        assert!(result.has_contradictions);
        assert!(result.contradiction_count > 0);
    }

    #[test]
    fn test_contradiction_detection_numeric() {
        let validator = CoherenceValidator::default_config();
        let segments = vec![
            "The temperature was 25 degrees.".to_string(),
            "The temperature was 75 degrees.".to_string(),
        ];

        let result = validator.detect_contradictions(&segments, None).unwrap();
        assert!(result.has_contradictions);
    }

    #[test]
    fn test_logical_flow() {
        let validator = CoherenceValidator::default_config();
        let segments = vec![
            "First, we need to analyze the data.".to_string(),
            "Then, we process the results.".to_string(),
            "Finally, we generate the report.".to_string(),
        ];

        let result = validator.check_logical_flow(&segments, None).unwrap();
        assert!(result.flow_score > 0.0);
        assert!(!result.transition_scores.is_empty());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_extract_numbers() {
        let numbers = extract_numbers("The value is 42.5 and -10");
        assert_eq!(numbers.len(), 2);
        assert!((numbers[0] - 42.5).abs() < 0.001);
        assert!((numbers[1] - (-10.0)).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity() {
        let sim = jaccard_similarity("hello world", "hello there world");
        assert!(sim > 0.5);
    }

    #[test]
    fn test_cache_operations() {
        let validator = CoherenceValidator::new(CoherenceConfig {
            enable_caching: true,
            ..Default::default()
        });

        // First call populates cache
        let _ = validator.compute_simple_embedding("test text");

        // Second call should use cache
        let _ = validator.compute_simple_embedding("test text");

        // Clear and verify
        validator.clear_cache();
        let cache = validator.embedding_cache.read();
        assert!(cache.is_empty());
    }
}

//! Diversity Analysis for Generated Content
//!
//! This module provides tools for analyzing diversity in generated content,
//! detecting mode collapse, and suggesting diversification strategies.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Configuration for diversity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiversityConfig {
    /// Minimum acceptable diversity score (0.0-1.0)
    pub min_diversity: f32,
    /// Mode collapse detection threshold (0.0-1.0)
    /// If average similarity exceeds this, mode collapse is detected
    pub mode_collapse_threshold: f32,
    /// Embedding dimension for diversity computation
    pub embedding_dim: usize,
    /// Number of n-grams to use for lexical diversity
    pub ngram_size: usize,
    /// Window size for rolling diversity calculation
    pub window_size: usize,
    /// Enable semantic diversity (requires embeddings)
    pub semantic_diversity: bool,
    /// Weight for lexical diversity in combined score
    pub lexical_weight: f32,
    /// Weight for semantic diversity in combined score
    pub semantic_weight: f32,
}

impl Default for DiversityConfig {
    fn default() -> Self {
        Self {
            min_diversity: 0.5,
            mode_collapse_threshold: 0.9,
            embedding_dim: 768,
            ngram_size: 3,
            window_size: 100,
            semantic_diversity: true,
            lexical_weight: 0.4,
            semantic_weight: 0.6,
        }
    }
}

/// Result of diversity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiversityResult {
    /// Overall diversity score (0.0-1.0)
    pub diversity_score: f32,
    /// Lexical diversity score (based on vocabulary and n-grams)
    pub lexical_diversity: f32,
    /// Semantic diversity score (based on embedding variance)
    pub semantic_diversity: f32,
    /// Type-token ratio
    pub type_token_ratio: f32,
    /// Unique n-gram ratio
    pub unique_ngram_ratio: f32,
    /// Embedding variance (if computed)
    pub embedding_variance: f32,
    /// Number of unique tokens
    pub unique_tokens: usize,
    /// Total tokens
    pub total_tokens: usize,
    /// Diversity by category (if applicable)
    pub category_diversity: HashMap<String, f32>,
}

impl Default for DiversityResult {
    fn default() -> Self {
        Self {
            diversity_score: 0.0,
            lexical_diversity: 0.0,
            semantic_diversity: 0.0,
            type_token_ratio: 0.0,
            unique_ngram_ratio: 0.0,
            embedding_variance: 0.0,
            unique_tokens: 0,
            total_tokens: 0,
            category_diversity: HashMap::new(),
        }
    }
}

/// Result of mode collapse detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeCollapseResult {
    /// Whether mode collapse is detected
    pub has_mode_collapse: bool,
    /// Severity of mode collapse (0.0-1.0, higher = worse)
    pub collapse_severity: f32,
    /// Average pairwise similarity
    pub average_similarity: f32,
    /// Percentage of samples in the dominant cluster
    pub dominant_cluster_percentage: f32,
    /// Repeated patterns detected
    pub repeated_patterns: Vec<RepeatedPattern>,
    /// Diagnosis message
    pub diagnosis: String,
}

impl Default for ModeCollapseResult {
    fn default() -> Self {
        Self {
            has_mode_collapse: false,
            collapse_severity: 0.0,
            average_similarity: 0.0,
            dominant_cluster_percentage: 0.0,
            repeated_patterns: Vec::new(),
            diagnosis: "No mode collapse detected".to_string(),
        }
    }
}

/// A repeated pattern found in content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatedPattern {
    /// The repeated text/pattern
    pub pattern: String,
    /// Number of occurrences
    pub count: usize,
    /// Indices where pattern appears
    pub occurrences: Vec<usize>,
}

/// Suggestion for improving diversity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiversificationSuggestion {
    /// Type of suggestion
    pub suggestion_type: SuggestionType,
    /// Human-readable suggestion
    pub message: String,
    /// Priority (higher = more important)
    pub priority: u8,
    /// Specific parameters to adjust
    pub parameters: HashMap<String, String>,
}

/// Types of diversification suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionType {
    /// Increase temperature parameter
    IncreaseTemperature,
    /// Adjust top-p sampling
    AdjustTopP,
    /// Adjust top-k sampling
    AdjustTopK,
    /// Use diverse beam search
    DiverseBeamSearch,
    /// Add prompt variation
    PromptVariation,
    /// Use different seed values
    SeedVariation,
    /// Apply penalty to repeated tokens
    RepetitionPenalty,
    /// Use nucleus sampling
    NucleusSampling,
    /// Add noise to embeddings
    EmbeddingNoise,
}

/// Diversity analyzer for generated content
pub struct DiversityAnalyzer {
    /// Configuration
    config: DiversityConfig,
    /// Historical samples for comparison
    history: Arc<RwLock<Vec<HistorySample>>>,
    /// N-gram cache
    ngram_cache: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

/// Historical sample for diversity tracking
#[derive(Clone)]
struct HistorySample {
    /// Sample text
    text: String,
    /// Embedding if available
    embedding: Option<Vec<f32>>,
    /// Timestamp
    timestamp: std::time::Instant,
}

impl DiversityAnalyzer {
    /// Create a new diversity analyzer
    pub fn new(config: DiversityConfig) -> Self {
        Self {
            config,
            history: Arc::new(RwLock::new(Vec::new())),
            ngram_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(DiversityConfig::default())
    }

    /// Calculate diversity score for a set of samples
    pub fn calculate_diversity(
        &self,
        samples: &[String],
        embeddings: Option<&[Vec<f32>]>,
    ) -> DiversityResult {
        if samples.is_empty() {
            return DiversityResult::default();
        }

        // Calculate lexical diversity
        let lexical = self.calculate_lexical_diversity(samples);

        // Calculate semantic diversity if embeddings provided
        let semantic = if let Some(emb) = embeddings {
            self.calculate_semantic_diversity(emb)
        } else if self.config.semantic_diversity {
            // Compute simple embeddings
            let simple_emb: Vec<Vec<f32>> = samples
                .iter()
                .map(|s| self.compute_simple_embedding(s))
                .collect();
            self.calculate_semantic_diversity(&simple_emb)
        } else {
            // Neutral score when semantic not computed
            SemanticDiversityResult {
                diversity_score: 0.5,
                variance: 0.0,
                average_distance: 0.0,
            }
        };

        // Calculate type-token ratio
        let (ttr, unique, total) = self.calculate_type_token_ratio(samples);

        // Calculate unique n-gram ratio
        let ngram_ratio = self.calculate_ngram_diversity(samples);

        // Combined diversity score
        let diversity_score = self.config.lexical_weight * lexical.diversity_score
            + self.config.semantic_weight * semantic.diversity_score;

        DiversityResult {
            diversity_score,
            lexical_diversity: lexical.diversity_score,
            semantic_diversity: semantic.diversity_score,
            type_token_ratio: ttr,
            unique_ngram_ratio: ngram_ratio,
            embedding_variance: semantic.variance,
            unique_tokens: unique,
            total_tokens: total,
            category_diversity: HashMap::new(),
        }
    }

    /// Detect mode collapse in generated samples
    pub fn detect_mode_collapse(
        &self,
        samples: &[String],
        embeddings: Option<&[Vec<f32>]>,
    ) -> ModeCollapseResult {
        if samples.len() < 2 {
            return ModeCollapseResult::default();
        }

        // Get embeddings
        let emb = match embeddings {
            Some(e) => e.to_vec(),
            None => samples
                .iter()
                .map(|s| self.compute_simple_embedding(s))
                .collect(),
        };

        // Calculate average pairwise similarity
        let mut total_sim = 0.0;
        let mut count = 0;

        for i in 0..emb.len() {
            for j in (i + 1)..emb.len() {
                total_sim += cosine_similarity(&emb[i], &emb[j]);
                count += 1;
            }
        }

        let avg_similarity = if count > 0 {
            total_sim / count as f32
        } else {
            0.0
        };

        // Detect repeated patterns
        let repeated_patterns = self.find_repeated_patterns(samples);

        // Simple clustering to find dominant mode
        let dominant_percentage = self.estimate_dominant_cluster(&emb);

        // Determine if mode collapse occurred
        let has_collapse = avg_similarity > self.config.mode_collapse_threshold
            || dominant_percentage > 0.7
            || repeated_patterns.len() > samples.len() / 4;

        let collapse_severity = if has_collapse {
            ((avg_similarity - self.config.mode_collapse_threshold)
                / (1.0 - self.config.mode_collapse_threshold))
                .clamp(0.0, 1.0)
                * 0.5
                + dominant_percentage * 0.3
                + (repeated_patterns.len() as f32 / samples.len() as f32).min(1.0) * 0.2
        } else {
            0.0
        };

        let diagnosis = if has_collapse {
            if avg_similarity > self.config.mode_collapse_threshold {
                format!(
                    "High similarity detected (avg: {:.2}). Samples are too similar.",
                    avg_similarity
                )
            } else if dominant_percentage > 0.7 {
                format!(
                    "Dominant cluster contains {:.0}% of samples.",
                    dominant_percentage * 100.0
                )
            } else {
                format!(
                    "Found {} repeated patterns indicating lack of diversity.",
                    repeated_patterns.len()
                )
            }
        } else {
            "No mode collapse detected".to_string()
        };

        ModeCollapseResult {
            has_mode_collapse: has_collapse,
            collapse_severity,
            average_similarity: avg_similarity,
            dominant_cluster_percentage: dominant_percentage,
            repeated_patterns,
            diagnosis,
        }
    }

    /// Suggest ways to improve diversity
    pub fn suggest_diversification(
        &self,
        diversity_result: &DiversityResult,
        mode_collapse: Option<&ModeCollapseResult>,
    ) -> Vec<DiversificationSuggestion> {
        let mut suggestions = Vec::new();

        // Low overall diversity
        if diversity_result.diversity_score < self.config.min_diversity {
            suggestions.push(DiversificationSuggestion {
                suggestion_type: SuggestionType::IncreaseTemperature,
                message: "Increase temperature parameter to add more randomness".to_string(),
                priority: 3,
                parameters: [("temperature".to_string(), "1.0-1.5".to_string())]
                    .into_iter()
                    .collect(),
            });
        }

        // Low lexical diversity
        if diversity_result.lexical_diversity < 0.4 {
            suggestions.push(DiversificationSuggestion {
                suggestion_type: SuggestionType::RepetitionPenalty,
                message: "Apply repetition penalty to avoid repeated phrases".to_string(),
                priority: 2,
                parameters: [("repetition_penalty".to_string(), "1.1-1.3".to_string())]
                    .into_iter()
                    .collect(),
            });
        }

        // Low semantic diversity
        if diversity_result.semantic_diversity < 0.4 {
            suggestions.push(DiversificationSuggestion {
                suggestion_type: SuggestionType::DiverseBeamSearch,
                message: "Use diverse beam search for more varied outputs".to_string(),
                priority: 2,
                parameters: [
                    ("num_beam_groups".to_string(), "4".to_string()),
                    ("diversity_penalty".to_string(), "0.5".to_string()),
                ]
                .into_iter()
                .collect(),
            });
        }

        // Mode collapse detected
        if let Some(collapse) = mode_collapse {
            if collapse.has_mode_collapse {
                suggestions.push(DiversificationSuggestion {
                    suggestion_type: SuggestionType::SeedVariation,
                    message: "Use different random seeds for each generation".to_string(),
                    priority: 3,
                    parameters: HashMap::new(),
                });

                suggestions.push(DiversificationSuggestion {
                    suggestion_type: SuggestionType::AdjustTopP,
                    message: "Adjust top-p (nucleus) sampling parameter".to_string(),
                    priority: 2,
                    parameters: [("top_p".to_string(), "0.9-0.95".to_string())]
                        .into_iter()
                        .collect(),
                });

                if collapse.collapse_severity > 0.5 {
                    suggestions.push(DiversificationSuggestion {
                        suggestion_type: SuggestionType::PromptVariation,
                        message: "Add variations to input prompts".to_string(),
                        priority: 3,
                        parameters: HashMap::new(),
                    });
                }
            }
        }

        // Low type-token ratio
        if diversity_result.type_token_ratio < 0.3 {
            suggestions.push(DiversificationSuggestion {
                suggestion_type: SuggestionType::AdjustTopK,
                message: "Increase top-k to sample from larger vocabulary".to_string(),
                priority: 1,
                parameters: [("top_k".to_string(), "50-100".to_string())]
                    .into_iter()
                    .collect(),
            });
        }

        // Sort by priority (higher first)
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions
    }

    /// Calculate lexical diversity
    fn calculate_lexical_diversity(&self, samples: &[String]) -> LexicalDiversityResult {
        let mut all_tokens = Vec::new();
        let mut all_bigrams = HashSet::new();
        let mut all_trigrams = HashSet::new();

        for sample in samples {
            let tokens: Vec<&str> = sample.split_whitespace().collect();
            all_tokens.extend(tokens.iter().map(|s| s.to_lowercase()));

            // Collect bigrams and trigrams
            for i in 0..tokens.len() {
                if i + 1 < tokens.len() {
                    all_bigrams.insert(format!("{} {}", tokens[i], tokens[i + 1]));
                }
                if i + 2 < tokens.len() {
                    all_trigrams.insert(format!(
                        "{} {} {}",
                        tokens[i],
                        tokens[i + 1],
                        tokens[i + 2]
                    ));
                }
            }
        }

        let unique_tokens: HashSet<String> = all_tokens.iter().cloned().collect();

        let ttr = if all_tokens.is_empty() {
            0.0
        } else {
            unique_tokens.len() as f32 / all_tokens.len() as f32
        };

        // Hapax legomena ratio (words appearing only once)
        let mut token_counts: HashMap<String, usize> = HashMap::new();
        for token in &all_tokens {
            *token_counts.entry(token.clone()).or_insert(0) += 1;
        }
        let hapax_count = token_counts.values().filter(|&&c| c == 1).count();
        let hapax_ratio = if unique_tokens.is_empty() {
            0.0
        } else {
            hapax_count as f32 / unique_tokens.len() as f32
        };

        // Combined lexical diversity score
        let diversity_score = (ttr * 0.4 + hapax_ratio * 0.3 + 0.3).min(1.0);

        LexicalDiversityResult {
            diversity_score,
            ttr,
            hapax_ratio,
            unique_bigrams: all_bigrams.len(),
            unique_trigrams: all_trigrams.len(),
        }
    }

    /// Calculate semantic diversity from embeddings
    fn calculate_semantic_diversity(&self, embeddings: &[Vec<f32>]) -> SemanticDiversityResult {
        if embeddings.is_empty() {
            return SemanticDiversityResult::default();
        }

        let dim = embeddings[0].len();
        let n = embeddings.len() as f32;

        // Calculate mean embedding
        let mut mean = vec![0.0f32; dim];
        for emb in embeddings {
            for (i, val) in emb.iter().enumerate() {
                mean[i] += val / n;
            }
        }

        // Calculate variance
        let mut variance = 0.0f32;
        for emb in embeddings {
            for (i, val) in emb.iter().enumerate() {
                variance += (val - mean[i]).powi(2);
            }
        }
        variance /= n * dim as f32;

        // Calculate pairwise diversity
        let mut total_distance = 0.0;
        let mut count = 0;

        for i in 0..embeddings.len() {
            for j in (i + 1)..embeddings.len() {
                let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                total_distance += 1.0 - sim; // Convert similarity to distance
                count += 1;
            }
        }

        let avg_distance = if count > 0 {
            total_distance / count as f32
        } else {
            0.0
        };

        // Diversity score based on average distance and variance
        let diversity_score = (avg_distance * 0.6 + variance.sqrt() * 0.4).min(1.0);

        SemanticDiversityResult {
            diversity_score,
            variance,
            average_distance: avg_distance,
        }
    }

    /// Calculate type-token ratio
    fn calculate_type_token_ratio(&self, samples: &[String]) -> (f32, usize, usize) {
        let mut all_tokens = Vec::new();

        for sample in samples {
            let tokens: Vec<String> = sample
                .split_whitespace()
                .map(|s| s.to_lowercase())
                .collect();
            all_tokens.extend(tokens);
        }

        let unique: HashSet<String> = all_tokens.iter().cloned().collect();
        let unique_count = unique.len();
        let total_count = all_tokens.len();

        let ttr = if total_count == 0 {
            0.0
        } else {
            unique_count as f32 / total_count as f32
        };

        (ttr, unique_count, total_count)
    }

    /// Calculate n-gram diversity
    fn calculate_ngram_diversity(&self, samples: &[String]) -> f32 {
        let mut all_ngrams = HashSet::new();
        let mut total_ngrams = 0;

        for sample in samples {
            let tokens: Vec<&str> = sample.split_whitespace().collect();

            for i in 0..tokens.len().saturating_sub(self.config.ngram_size - 1) {
                let ngram: String = tokens[i..i + self.config.ngram_size].join(" ");
                all_ngrams.insert(ngram);
                total_ngrams += 1;
            }
        }

        if total_ngrams == 0 {
            return 0.0;
        }

        all_ngrams.len() as f32 / total_ngrams as f32
    }

    /// Find repeated patterns in samples
    fn find_repeated_patterns(&self, samples: &[String]) -> Vec<RepeatedPattern> {
        let mut patterns: HashMap<String, Vec<usize>> = HashMap::new();

        // Look for repeated n-grams
        for (idx, sample) in samples.iter().enumerate() {
            let tokens: Vec<&str> = sample.split_whitespace().collect();

            for n in 3..=5 {
                for i in 0..tokens.len().saturating_sub(n - 1) {
                    let ngram: String = tokens[i..i + n].join(" ");
                    patterns.entry(ngram).or_default().push(idx);
                }
            }
        }

        // Filter to patterns appearing multiple times
        patterns
            .into_iter()
            .filter(|(_, indices)| indices.len() >= 2)
            .map(|(pattern, occurrences)| RepeatedPattern {
                pattern,
                count: occurrences.len(),
                occurrences,
            })
            .collect()
    }

    /// Estimate dominant cluster percentage
    fn estimate_dominant_cluster(&self, embeddings: &[Vec<f32>]) -> f32 {
        if embeddings.len() < 3 {
            return 1.0;
        }

        // Simple approach: find the percentage of samples within threshold of the centroid
        let dim = embeddings[0].len();
        let n = embeddings.len() as f32;

        // Calculate centroid
        let mut centroid = vec![0.0f32; dim];
        for emb in embeddings {
            for (i, val) in emb.iter().enumerate() {
                centroid[i] += val / n;
            }
        }

        // Count samples close to centroid
        let threshold = 0.8;
        let close_count = embeddings
            .iter()
            .filter(|emb| cosine_similarity(emb, &centroid) > threshold)
            .count();

        close_count as f32 / embeddings.len() as f32
    }

    /// Compute simple embedding for text
    fn compute_simple_embedding(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.config.embedding_dim];
        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();

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

        embedding
    }

    /// Add sample to history for tracking
    pub fn add_to_history(&self, text: String, embedding: Option<Vec<f32>>) {
        let mut history = self.history.write();

        // Limit history size
        while history.len() >= self.config.window_size {
            history.remove(0);
        }

        history.push(HistorySample {
            text,
            embedding,
            timestamp: std::time::Instant::now(),
        });
    }

    /// Get rolling diversity from history
    pub fn get_rolling_diversity(&self) -> DiversityResult {
        let history = self.history.read();

        if history.is_empty() {
            return DiversityResult::default();
        }

        let texts: Vec<String> = history.iter().map(|s| s.text.clone()).collect();
        let embeddings: Option<Vec<Vec<f32>>> = if history.iter().all(|s| s.embedding.is_some()) {
            Some(history.iter().filter_map(|s| s.embedding.clone()).collect())
        } else {
            None
        };

        self.calculate_diversity(&texts, embeddings.as_deref())
    }

    /// Clear history
    pub fn clear_history(&self) {
        let mut history = self.history.write();
        history.clear();
    }
}

/// Internal result for lexical diversity
struct LexicalDiversityResult {
    diversity_score: f32,
    ttr: f32,
    hapax_ratio: f32,
    unique_bigrams: usize,
    unique_trigrams: usize,
}

/// Internal result for semantic diversity
#[derive(Default)]
struct SemanticDiversityResult {
    diversity_score: f32,
    variance: f32,
    average_distance: f32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diversity_calculation() {
        let analyzer = DiversityAnalyzer::default_config();

        let samples = vec![
            "The quick brown fox jumps over the lazy dog.".to_string(),
            "A fast red cat leaps across the sleepy hound.".to_string(),
            "The swift grey wolf runs past the tired sheep.".to_string(),
        ];

        let result = analyzer.calculate_diversity(&samples, None);
        assert!(result.diversity_score > 0.0);
        assert!(result.lexical_diversity > 0.0);
    }

    #[test]
    fn test_mode_collapse_detection_similar() {
        let analyzer = DiversityAnalyzer::default_config();

        let samples = vec![
            "The cat sat on the mat.".to_string(),
            "The cat sat on the mat.".to_string(),
            "The cat sat on the mat.".to_string(),
            "The cat sat on the mat.".to_string(),
        ];

        let result = analyzer.detect_mode_collapse(&samples, None);
        assert!(result.has_mode_collapse);
        assert!(result.average_similarity > 0.9);
    }

    #[test]
    fn test_mode_collapse_detection_diverse() {
        let analyzer = DiversityAnalyzer::default_config();

        let samples = vec![
            "The weather is sunny today.".to_string(),
            "I enjoy programming in Rust.".to_string(),
            "Machine learning is fascinating.".to_string(),
            "The ocean waves are calming.".to_string(),
        ];

        let result = analyzer.detect_mode_collapse(&samples, None);
        // These are quite different topics, so mode collapse should be lower
        assert!(result.collapse_severity < 0.8);
    }

    #[test]
    fn test_diversification_suggestions() {
        let analyzer = DiversityAnalyzer::default_config();

        let low_diversity = DiversityResult {
            diversity_score: 0.2,
            lexical_diversity: 0.3,
            semantic_diversity: 0.2,
            type_token_ratio: 0.2,
            ..Default::default()
        };

        let suggestions = analyzer.suggest_diversification(&low_diversity, None);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_type_token_ratio() {
        let analyzer = DiversityAnalyzer::default_config();

        let samples = vec![
            "one two three four five".to_string(),
            "one one one one one".to_string(),
        ];

        let (ttr, unique, total) = analyzer.calculate_type_token_ratio(&samples);
        assert_eq!(total, 10);
        assert_eq!(unique, 5);
        assert!((ttr - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_repeated_patterns() {
        let analyzer = DiversityAnalyzer::default_config();

        let samples = vec![
            "the quick brown fox".to_string(),
            "the quick brown cat".to_string(),
            "the quick brown dog".to_string(),
        ];

        let patterns = analyzer.find_repeated_patterns(&samples);
        assert!(!patterns.is_empty());

        // "the quick brown" should be repeated
        let found = patterns.iter().any(|p| p.pattern == "the quick brown");
        assert!(found);
    }

    #[test]
    fn test_history_tracking() {
        let analyzer = DiversityAnalyzer::new(DiversityConfig {
            window_size: 5,
            ..Default::default()
        });

        for i in 0..10 {
            analyzer.add_to_history(format!("Sample text number {}", i), None);
        }

        let history = analyzer.history.read();
        assert_eq!(history.len(), 5); // Should be limited to window_size
    }

    #[test]
    fn test_rolling_diversity() {
        let analyzer = DiversityAnalyzer::default_config();

        analyzer.add_to_history("First unique sentence about cats.".to_string(), None);
        analyzer.add_to_history("Second different statement about dogs.".to_string(), None);
        analyzer.add_to_history("Third varied text about birds.".to_string(), None);

        let result = analyzer.get_rolling_diversity();
        assert!(result.diversity_score > 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }
}

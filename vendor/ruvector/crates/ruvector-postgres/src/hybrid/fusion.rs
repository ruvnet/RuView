//! Fusion algorithms for combining vector and keyword search results
//!
//! Provides:
//! - RRF (Reciprocal Rank Fusion) - default, robust
//! - Linear blend - simple weighted combination
//! - Learned fusion - query-adaptive weights

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Document ID type (matches with database row IDs)
pub type DocId = i64;

/// Default RRF constant
pub const DEFAULT_RRF_K: usize = 60;

/// Default alpha for linear fusion (0.5 = equal weight)
pub const DEFAULT_ALPHA: f32 = 0.5;

/// Fusion method selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FusionMethod {
    /// Reciprocal Rank Fusion (default)
    Rrf,
    /// Linear weighted combination
    Linear,
    /// Learned/adaptive fusion based on query features
    Learned,
}

impl Default for FusionMethod {
    fn default() -> Self {
        FusionMethod::Rrf
    }
}

impl std::str::FromStr for FusionMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rrf" | "reciprocal" | "reciprocal_rank" => Ok(FusionMethod::Rrf),
            "linear" | "blend" | "weighted" => Ok(FusionMethod::Linear),
            "learned" | "adaptive" | "auto" => Ok(FusionMethod::Learned),
            _ => Err(format!("Unknown fusion method: {}", s)),
        }
    }
}

/// Fusion configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Fusion method to use
    pub method: FusionMethod,
    /// RRF constant (typically 60)
    pub rrf_k: usize,
    /// Alpha for linear fusion (0 = all keyword, 1 = all vector)
    pub alpha: f32,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            method: FusionMethod::Rrf,
            rrf_k: DEFAULT_RRF_K,
            alpha: DEFAULT_ALPHA,
        }
    }
}

/// Search result from a single branch (vector or keyword)
#[derive(Debug, Clone)]
pub struct BranchResult {
    /// Document ID
    pub doc_id: DocId,
    /// Original score from the branch
    pub score: f32,
    /// Rank in the result set (0-indexed)
    pub rank: usize,
}

/// Fused search result combining both branches
#[derive(Debug, Clone)]
pub struct FusedResult {
    /// Document ID
    pub doc_id: DocId,
    /// Final fused score
    pub hybrid_score: f32,
    /// Original vector score (if present)
    pub vector_score: Option<f32>,
    /// Original keyword score (if present)
    pub keyword_score: Option<f32>,
}

/// Reciprocal Rank Fusion (RRF)
///
/// RRF score = sum over sources: 1 / (k + rank)
///
/// Properties:
/// - Robust to different score scales
/// - No need for score calibration
/// - Works well with partial overlap
///
/// Reference: "Reciprocal Rank Fusion outperforms Condorcet and individual Rank Learning Methods"
pub fn rrf_fusion(
    vector_results: &[(DocId, f32)],  // (id, distance - lower is better)
    keyword_results: &[(DocId, f32)], // (id, BM25 score - higher is better)
    k: usize,
    limit: usize,
) -> Vec<FusedResult> {
    let mut scores: HashMap<DocId, (f32, Option<f32>, Option<f32>)> = HashMap::new();

    // Vector ranking (lower distance = higher rank, so already sorted best first)
    for (rank, (doc_id, distance)) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank + 1) as f32;
        let entry = scores.entry(*doc_id).or_insert((0.0, None, None));
        entry.0 += rrf_score;
        entry.1 = Some(*distance);
    }

    // Keyword ranking (higher BM25 = higher rank, already sorted best first)
    for (rank, (doc_id, bm25_score)) in keyword_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + rank + 1) as f32;
        let entry = scores.entry(*doc_id).or_insert((0.0, None, None));
        entry.0 += rrf_score;
        entry.2 = Some(*bm25_score);
    }

    // Sort by fused score (descending)
    let mut results: Vec<FusedResult> = scores
        .into_iter()
        .map(
            |(doc_id, (hybrid_score, vector_score, keyword_score))| FusedResult {
                doc_id,
                hybrid_score,
                vector_score,
                keyword_score,
            },
        )
        .collect();

    results.sort_by(|a, b| {
        b.hybrid_score
            .partial_cmp(&a.hybrid_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

/// Normalize vector distances to similarity scores [0, 1]
///
/// Converts distance (lower = better) to similarity (higher = better)
fn normalize_to_similarity(results: &[(DocId, f32)]) -> Vec<(DocId, f32)> {
    if results.is_empty() {
        return Vec::new();
    }

    // Find min/max distances
    let (min_dist, max_dist) = results
        .iter()
        .fold((f32::MAX, f32::MIN), |(min, max), (_, d)| {
            (min.min(*d), max.max(*d))
        });

    let range = (max_dist - min_dist).max(1e-6);

    results
        .iter()
        .map(|(id, dist)| {
            // Convert distance to similarity: 1 - normalized_distance
            let similarity = 1.0 - (dist - min_dist) / range;
            (*id, similarity)
        })
        .collect()
}

/// Min-max normalize scores to [0, 1]
fn min_max_normalize(results: &[(DocId, f32)]) -> Vec<(DocId, f32)> {
    if results.is_empty() {
        return Vec::new();
    }

    let (min_score, max_score) = results
        .iter()
        .fold((f32::MAX, f32::MIN), |(min, max), (_, s)| {
            (min.min(*s), max.max(*s))
        });

    let range = (max_score - min_score).max(1e-6);

    results
        .iter()
        .map(|(id, score)| {
            let normalized = (score - min_score) / range;
            (*id, normalized)
        })
        .collect()
}

/// Linear fusion with alpha blending
///
/// score = alpha * vector_similarity + (1 - alpha) * keyword_score
///
/// Note: Scores must be normalized before fusion
pub fn linear_fusion(
    vector_results: &[(DocId, f32)],  // (id, distance)
    keyword_results: &[(DocId, f32)], // (id, BM25 score)
    alpha: f32,
    limit: usize,
) -> Vec<FusedResult> {
    // Normalize vector scores (distance -> similarity)
    let vec_scores: HashMap<DocId, f32> = normalize_to_similarity(vector_results)
        .into_iter()
        .collect();

    // Normalize keyword scores to [0, 1]
    let kw_scores: HashMap<DocId, f32> = min_max_normalize(keyword_results).into_iter().collect();

    // Combine scores
    let mut combined: HashMap<DocId, (f32, Option<f32>, Option<f32>)> = HashMap::new();

    // Add vector results
    for (doc_id, sim) in &vec_scores {
        let entry = combined.entry(*doc_id).or_insert((0.0, None, None));
        entry.0 += alpha * sim;
        // Store original distance
        if let Some((_, dist)) = vector_results.iter().find(|(id, _)| id == doc_id) {
            entry.1 = Some(*dist);
        }
    }

    // Add keyword results
    for (doc_id, norm_score) in &kw_scores {
        let entry = combined.entry(*doc_id).or_insert((0.0, None, None));
        entry.0 += (1.0 - alpha) * norm_score;
        // Store original BM25 score
        if let Some((_, score)) = keyword_results.iter().find(|(id, _)| id == doc_id) {
            entry.2 = Some(*score);
        }
    }

    // Sort by fused score
    let mut results: Vec<FusedResult> = combined
        .into_iter()
        .map(
            |(doc_id, (hybrid_score, vector_score, keyword_score))| FusedResult {
                doc_id,
                hybrid_score,
                vector_score,
                keyword_score,
            },
        )
        .collect();

    results.sort_by(|a, b| {
        b.hybrid_score
            .partial_cmp(&a.hybrid_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

/// Query features for learned fusion
#[derive(Debug, Clone)]
pub struct QueryFeatures {
    /// L2 norm of query embedding
    pub embedding_norm: f32,
    /// Number of query terms
    pub term_count: usize,
    /// Average IDF of query terms
    pub avg_term_idf: f32,
    /// Whether query appears to need exact matching
    pub has_exact_match: bool,
    /// Classified query type
    pub query_type: QueryType,
}

/// Query type classification for learned fusion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Navigational query (looking for specific entity)
    Navigational,
    /// Informational query (seeking information)
    Informational,
    /// Transactional query (action-oriented)
    Transactional,
    /// Unknown/mixed
    Unknown,
}

/// Simple fusion model for learned/adaptive fusion
///
/// In production, this would be a trained ML model (e.g., GNN, logistic regression)
pub struct FusionModel {
    /// Default alpha when model can't make prediction
    pub default_alpha: f32,
    /// Weight for embedding norm
    pub norm_weight: f32,
    /// Weight for term count
    pub term_weight: f32,
    /// Weight for avg IDF
    pub idf_weight: f32,
    /// Bias for exact match queries (favor keyword)
    pub exact_match_bias: f32,
}

impl Default for FusionModel {
    fn default() -> Self {
        Self {
            default_alpha: 0.5,
            norm_weight: 0.1,
            term_weight: -0.05,     // More terms -> slight keyword preference
            idf_weight: 0.15,       // Rare terms -> vector preference
            exact_match_bias: -0.2, // Exact match -> keyword preference
        }
    }
}

impl FusionModel {
    /// Predict optimal alpha for a query
    pub fn predict_alpha(&self, features: &QueryFeatures) -> f32 {
        let mut alpha = self.default_alpha;

        // Adjust based on embedding norm (high norm -> more distinctive)
        alpha += self.norm_weight * (features.embedding_norm - 1.0).clamp(-1.0, 1.0);

        // Adjust based on term count
        alpha += self.term_weight * (features.term_count as f32 - 3.0).clamp(-3.0, 3.0);

        // Adjust based on avg IDF (high IDF = rare terms, favor vector)
        alpha += self.idf_weight * (features.avg_term_idf - 3.0).clamp(-3.0, 3.0);

        // Adjust for exact match intent
        if features.has_exact_match {
            alpha += self.exact_match_bias;
        }

        // Adjust based on query type
        match features.query_type {
            QueryType::Navigational => alpha -= 0.15, // Favor keyword
            QueryType::Informational => alpha += 0.1, // Favor vector
            QueryType::Transactional => alpha -= 0.05,
            QueryType::Unknown => {}
        }

        // Clamp to valid range
        alpha.clamp(0.0, 1.0)
    }
}

/// Learned fusion using query characteristics
pub fn learned_fusion(
    query_embedding: &[f32],
    query_terms: &[String],
    vector_results: &[(DocId, f32)],
    keyword_results: &[(DocId, f32)],
    model: &FusionModel,
    avg_term_idf: f32, // Pre-computed average IDF
    limit: usize,
) -> Vec<FusedResult> {
    // Compute query features
    let embedding_norm = l2_norm(query_embedding);
    let has_exact_match = detect_exact_match_intent(query_terms);
    let query_type = classify_query_type(query_terms);

    let features = QueryFeatures {
        embedding_norm,
        term_count: query_terms.len(),
        avg_term_idf,
        has_exact_match,
        query_type,
    };

    // Predict optimal alpha
    let alpha = model.predict_alpha(&features);

    // Use linear fusion with predicted alpha
    linear_fusion(vector_results, keyword_results, alpha, limit)
}

/// Compute L2 norm of a vector
fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Detect if query seems to need exact matching
fn detect_exact_match_intent(terms: &[String]) -> bool {
    // Heuristics for exact match intent:
    // - Quoted phrases (handled upstream)
    // - Product codes, error codes, IDs
    // - Very short queries (1-2 terms)

    if terms.len() <= 2 {
        return true;
    }

    terms.iter().any(|t| {
        // Looks like a code/ID
        t.chars().any(|c| c.is_numeric()) && t.len() >= 3 && t.len() <= 20
    })
}

/// Classify query type based on terms
fn classify_query_type(terms: &[String]) -> QueryType {
    let terms_lower: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();

    // Navigational indicators
    let nav_indicators = ["website", "login", "home", "official", "download"];
    if terms_lower
        .iter()
        .any(|t| nav_indicators.contains(&t.as_str()))
    {
        return QueryType::Navigational;
    }

    // Transactional indicators
    let trans_indicators = ["buy", "purchase", "order", "price", "cheap", "best", "deal"];
    if terms_lower
        .iter()
        .any(|t| trans_indicators.contains(&t.as_str()))
    {
        return QueryType::Transactional;
    }

    // Informational indicators
    let info_indicators = [
        "how", "what", "why", "when", "where", "guide", "tutorial", "explain",
    ];
    if terms_lower
        .iter()
        .any(|t| info_indicators.contains(&t.as_str()))
    {
        return QueryType::Informational;
    }

    QueryType::Unknown
}

/// Fuse results using the specified method
pub fn fuse_results(
    vector_results: &[(DocId, f32)],
    keyword_results: &[(DocId, f32)],
    config: &FusionConfig,
    limit: usize,
) -> Vec<FusedResult> {
    match config.method {
        FusionMethod::Rrf => rrf_fusion(vector_results, keyword_results, config.rrf_k, limit),
        FusionMethod::Linear => linear_fusion(vector_results, keyword_results, config.alpha, limit),
        FusionMethod::Learned => {
            // Learned fusion requires additional context
            // Fall back to RRF if no model is available
            rrf_fusion(vector_results, keyword_results, config.rrf_k, limit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vector_results() -> Vec<(DocId, f32)> {
        vec![
            (1, 0.1), // Best (lowest distance)
            (2, 0.2),
            (3, 0.3),
            (4, 0.5),
            (5, 0.8),
        ]
    }

    fn sample_keyword_results() -> Vec<(DocId, f32)> {
        vec![
            (3, 8.5), // Best (highest BM25)
            (1, 7.2),
            (6, 5.0),
            (2, 3.5),
            (7, 2.0),
        ]
    }

    #[test]
    fn test_rrf_fusion() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        let results = rrf_fusion(&vector, &keyword, 60, 5);

        assert!(!results.is_empty());
        // Doc 1 and 3 should be near top (they appear in both)
        let top_ids: Vec<DocId> = results.iter().map(|r| r.doc_id).collect();
        assert!(top_ids.contains(&1) || top_ids.contains(&3));
    }

    #[test]
    fn test_linear_fusion_alpha_1() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        // Alpha = 1.0 means only vector
        let results = linear_fusion(&vector, &keyword, 1.0, 5);

        assert!(!results.is_empty());
        // With alpha=1, vector-only docs should dominate
        let first = &results[0];
        assert!(first.vector_score.is_some());
    }

    #[test]
    fn test_linear_fusion_alpha_0() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        // Alpha = 0.0 means only keyword
        let results = linear_fusion(&vector, &keyword, 0.0, 5);

        assert!(!results.is_empty());
        // With alpha=0, keyword-only docs can appear
        let first = &results[0];
        assert!(first.keyword_score.is_some());
    }

    #[test]
    fn test_normalization() {
        let results = vec![(1, 0.1), (2, 0.5), (3, 0.9)];
        let normalized = normalize_to_similarity(&results);

        assert_eq!(normalized.len(), 3);
        // Lowest distance should have highest similarity
        let (id1, sim1) = normalized[0];
        assert_eq!(id1, 1);
        assert!((sim1 - 1.0).abs() < 0.01, "Best should be ~1.0");
    }

    #[test]
    fn test_fusion_model() {
        let model = FusionModel::default();

        // Short navigational query
        let features = QueryFeatures {
            embedding_norm: 1.0,
            term_count: 2,
            avg_term_idf: 2.0,
            has_exact_match: true,
            query_type: QueryType::Navigational,
        };

        let alpha = model.predict_alpha(&features);
        assert!(
            alpha < 0.5,
            "Navigational query should favor keyword (alpha < 0.5)"
        );

        // Long informational query
        let features2 = QueryFeatures {
            embedding_norm: 1.2,
            term_count: 6,
            avg_term_idf: 5.0,
            has_exact_match: false,
            query_type: QueryType::Informational,
        };

        let alpha2 = model.predict_alpha(&features2);
        assert!(
            alpha2 > 0.4,
            "Informational query with rare terms should favor vector"
        );
    }

    #[test]
    fn test_query_type_classification() {
        let nav = classify_query_type(&["github".into(), "login".into()]);
        assert_eq!(nav, QueryType::Navigational);

        let info = classify_query_type(&["how".into(), "to".into(), "cook".into(), "pasta".into()]);
        assert_eq!(info, QueryType::Informational);

        let trans = classify_query_type(&["buy".into(), "laptop".into()]);
        assert_eq!(trans, QueryType::Transactional);
    }

    #[test]
    fn test_exact_match_detection() {
        assert!(detect_exact_match_intent(&["ERR001".into()]));
        assert!(detect_exact_match_intent(&["SKU12345".into()]));
        assert!(!detect_exact_match_intent(&[
            "database".into(),
            "connection".into(),
            "error".into(),
            "handling".into()
        ]));
    }

    #[test]
    fn test_empty_results() {
        let results = rrf_fusion(&[], &[], 60, 10);
        assert!(results.is_empty());

        let results2 = linear_fusion(&[], &[], 0.5, 10);
        assert!(results2.is_empty());
    }
}

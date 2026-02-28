//! Hybrid Search: Combining Vector Similarity and Keyword Matching
//!
//! Implements hybrid search by combining:
//! - Vector similarity search (semantic)
//! - BM25 keyword matching (lexical)
//! - Weighted combination of scores

use crate::error::Result;
use crate::types::{SearchResult, VectorId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for hybrid search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfig {
    /// Weight for vector similarity (alpha)
    pub vector_weight: f32,
    /// Weight for keyword matching (beta)
    pub keyword_weight: f32,
    /// Normalization strategy
    pub normalization: NormalizationStrategy,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.7,
            keyword_weight: 0.3,
            normalization: NormalizationStrategy::MinMax,
        }
    }
}

/// Score normalization strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalizationStrategy {
    /// Min-max normalization: (x - min) / (max - min)
    MinMax,
    /// Z-score normalization: (x - mean) / std
    ZScore,
    /// No normalization
    None,
}

/// Simple BM25 implementation for keyword matching
#[derive(Debug, Clone)]
pub struct BM25 {
    /// IDF scores for terms
    pub idf: HashMap<String, f32>,
    /// Average document length
    pub avg_doc_len: f32,
    /// Document lengths
    pub doc_lengths: HashMap<VectorId, usize>,
    /// Inverted index: term -> set of doc IDs
    pub inverted_index: HashMap<String, HashSet<VectorId>>,
    /// BM25 parameters
    pub k1: f32,
    pub b: f32,
}

impl BM25 {
    /// Create a new BM25 instance
    pub fn new(k1: f32, b: f32) -> Self {
        Self {
            idf: HashMap::new(),
            avg_doc_len: 0.0,
            doc_lengths: HashMap::new(),
            inverted_index: HashMap::new(),
            k1,
            b,
        }
    }

    /// Index a document
    pub fn index_document(&mut self, doc_id: VectorId, text: &str) {
        let terms = tokenize(text);
        self.doc_lengths.insert(doc_id.clone(), terms.len());

        // Update inverted index
        for term in terms {
            self.inverted_index
                .entry(term)
                .or_default()
                .insert(doc_id.clone());
        }

        // Update average document length
        let total_len: usize = self.doc_lengths.values().sum();
        self.avg_doc_len = total_len as f32 / self.doc_lengths.len() as f32;
    }

    /// Build IDF scores after indexing all documents
    pub fn build_idf(&mut self) {
        let num_docs = self.doc_lengths.len() as f32;

        for (term, doc_set) in &self.inverted_index {
            let doc_freq = doc_set.len() as f32;
            let idf = ((num_docs - doc_freq + 0.5) / (doc_freq + 0.5) + 1.0).ln();
            self.idf.insert(term.clone(), idf);
        }
    }

    /// Compute BM25 score for a query against a document
    pub fn score(&self, query: &str, doc_id: &VectorId, doc_text: &str) -> f32 {
        let query_terms = tokenize(query);
        let doc_terms = tokenize(doc_text);
        let doc_len = self.doc_lengths.get(doc_id).copied().unwrap_or(0) as f32;

        // Count term frequencies in document
        let mut term_freq: HashMap<String, f32> = HashMap::new();
        for term in doc_terms {
            *term_freq.entry(term).or_insert(0.0) += 1.0;
        }

        // Calculate BM25 score
        let mut score = 0.0;
        for term in query_terms {
            let idf = self.idf.get(&term).copied().unwrap_or(0.0);
            let tf = term_freq.get(&term).copied().unwrap_or(0.0);

            let numerator = tf * (self.k1 + 1.0);
            let denominator = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len));

            score += idf * (numerator / denominator);
        }

        score
    }

    /// Get all documents containing at least one query term
    pub fn get_candidate_docs(&self, query: &str) -> HashSet<VectorId> {
        let query_terms = tokenize(query);
        let mut candidates = HashSet::new();

        for term in query_terms {
            if let Some(doc_set) = self.inverted_index.get(&term) {
                candidates.extend(doc_set.iter().cloned());
            }
        }

        candidates
    }
}

/// Hybrid search combining vector and keyword matching
#[derive(Debug, Clone)]
pub struct HybridSearch {
    /// Configuration
    pub config: HybridConfig,
    /// BM25 index for keyword matching
    pub bm25: BM25,
    /// Document texts for BM25 scoring
    pub doc_texts: HashMap<VectorId, String>,
}

impl HybridSearch {
    /// Create a new hybrid search instance
    pub fn new(config: HybridConfig) -> Self {
        Self {
            config,
            bm25: BM25::new(1.5, 0.75), // Standard BM25 parameters
            doc_texts: HashMap::new(),
        }
    }

    /// Index a document with both vector and text
    pub fn index_document(&mut self, doc_id: VectorId, text: String) {
        self.bm25.index_document(doc_id.clone(), &text);
        self.doc_texts.insert(doc_id, text);
    }

    /// Finalize indexing (build IDF scores)
    pub fn finalize_indexing(&mut self) {
        self.bm25.build_idf();
    }

    /// Perform hybrid search
    ///
    /// # Arguments
    /// * `query_vector` - Query vector for semantic search
    /// * `query_text` - Query text for keyword matching
    /// * `k` - Number of results to return
    /// * `vector_search_fn` - Function to perform vector similarity search
    ///
    /// # Returns
    /// Combined and reranked search results
    pub fn search<F>(
        &self,
        query_vector: &[f32],
        query_text: &str,
        k: usize,
        vector_search_fn: F,
    ) -> Result<Vec<SearchResult>>
    where
        F: Fn(&[f32], usize) -> Result<Vec<SearchResult>>,
    {
        // Get vector similarity results
        let vector_results = vector_search_fn(query_vector, k * 2)?;

        // Get keyword matching candidates
        let keyword_candidates = self.bm25.get_candidate_docs(query_text);

        // Compute BM25 scores for all candidates
        let mut bm25_scores: HashMap<VectorId, f32> = HashMap::new();
        for doc_id in &keyword_candidates {
            if let Some(doc_text) = self.doc_texts.get(doc_id) {
                let score = self.bm25.score(query_text, doc_id, doc_text);
                bm25_scores.insert(doc_id.clone(), score);
            }
        }

        // Combine results
        let mut combined_results: HashMap<VectorId, CombinedScore> = HashMap::new();

        // Add vector results
        for result in vector_results {
            combined_results.insert(
                result.id.clone(),
                CombinedScore {
                    id: result.id.clone(),
                    vector_score: Some(result.score),
                    keyword_score: bm25_scores.get(&result.id).copied(),
                    vector: result.vector,
                    metadata: result.metadata,
                },
            );
        }

        // Add keyword-only results
        for (doc_id, bm25_score) in bm25_scores {
            combined_results
                .entry(doc_id.clone())
                .or_insert(CombinedScore {
                    id: doc_id,
                    vector_score: None,
                    keyword_score: Some(bm25_score),
                    vector: None,
                    metadata: None,
                });
        }

        // Normalize and combine scores
        let normalized_results =
            self.normalize_and_combine(combined_results.into_values().collect())?;

        // Sort by combined score (descending)
        let mut sorted_results = normalized_results;
        sorted_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Return top-k
        Ok(sorted_results.into_iter().take(k).collect())
    }

    /// Normalize and combine scores
    fn normalize_and_combine(&self, results: Vec<CombinedScore>) -> Result<Vec<SearchResult>> {
        let mut vector_scores: Vec<f32> = results.iter().filter_map(|r| r.vector_score).collect();
        let mut keyword_scores: Vec<f32> = results.iter().filter_map(|r| r.keyword_score).collect();

        // Normalize scores
        normalize_scores(&mut vector_scores, self.config.normalization);
        normalize_scores(&mut keyword_scores, self.config.normalization);

        // Create lookup maps
        let mut vector_map: HashMap<VectorId, f32> = HashMap::new();
        let mut keyword_map: HashMap<VectorId, f32> = HashMap::new();

        for (result, &norm_score) in results.iter().zip(&vector_scores) {
            if result.vector_score.is_some() {
                vector_map.insert(result.id.clone(), norm_score);
            }
        }

        for (result, &norm_score) in results.iter().zip(&keyword_scores) {
            if result.keyword_score.is_some() {
                keyword_map.insert(result.id.clone(), norm_score);
            }
        }

        // Combine scores
        let combined: Vec<SearchResult> = results
            .into_iter()
            .map(|result| {
                let vector_norm = vector_map.get(&result.id).copied().unwrap_or(0.0);
                let keyword_norm = keyword_map.get(&result.id).copied().unwrap_or(0.0);

                let combined_score = self.config.vector_weight * vector_norm
                    + self.config.keyword_weight * keyword_norm;

                SearchResult {
                    id: result.id,
                    score: combined_score,
                    vector: result.vector,
                    metadata: result.metadata,
                }
            })
            .collect();

        Ok(combined)
    }
}

/// Combined score holder
#[derive(Debug, Clone)]
struct CombinedScore {
    id: VectorId,
    vector_score: Option<f32>,
    keyword_score: Option<f32>,
    vector: Option<Vec<f32>>,
    metadata: Option<HashMap<String, serde_json::Value>>,
}

// Helper functions

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 2) // Remove very short tokens
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_scores(scores: &mut [f32], strategy: NormalizationStrategy) {
    if scores.is_empty() {
        return;
    }

    match strategy {
        NormalizationStrategy::MinMax => {
            let min = scores.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = scores.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let range = max - min;

            if range > 0.0 {
                for score in scores.iter_mut() {
                    *score = (*score - min) / range;
                }
            }
        }
        NormalizationStrategy::ZScore => {
            let mean = scores.iter().sum::<f32>() / scores.len() as f32;
            let variance =
                scores.iter().map(|&s| (s - mean).powi(2)).sum::<f32>() / scores.len() as f32;
            let std = variance.sqrt();

            if std > 0.0 {
                for score in scores.iter_mut() {
                    *score = (*score - mean) / std;
                }
            }
        }
        NormalizationStrategy::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let text = "The quick brown fox jumps over the lazy dog!";
        let tokens = tokenize(text);
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"the".to_string())); // "the" is 3 chars, passes > 2 filter
        assert!(!tokens.contains(&"a".to_string())); // 1 char, too short
    }

    #[test]
    fn test_bm25_indexing() {
        let mut bm25 = BM25::new(1.5, 0.75);

        bm25.index_document("doc1".to_string(), "rust programming language");
        bm25.index_document("doc2".to_string(), "python programming tutorial");
        bm25.build_idf();

        assert_eq!(bm25.doc_lengths.len(), 2);
        assert!(bm25.idf.contains_key("rust"));
        assert!(bm25.idf.contains_key("programming"));
    }

    #[test]
    fn test_bm25_scoring() {
        let mut bm25 = BM25::new(1.5, 0.75);

        bm25.index_document("doc1".to_string(), "rust programming language");
        bm25.index_document("doc2".to_string(), "python programming tutorial");
        bm25.index_document("doc3".to_string(), "rust systems programming");
        bm25.build_idf();

        let score1 = bm25.score(
            "rust programming",
            &"doc1".to_string(),
            "rust programming language",
        );
        let score2 = bm25.score(
            "rust programming",
            &"doc2".to_string(),
            "python programming tutorial",
        );

        // doc1 should score higher (contains both terms)
        assert!(score1 > score2);
    }

    #[test]
    fn test_hybrid_search_initialization() {
        let config = HybridConfig::default();
        let mut hybrid = HybridSearch::new(config);

        hybrid.index_document("doc1".to_string(), "rust vector database".to_string());
        hybrid.index_document("doc2".to_string(), "python machine learning".to_string());
        hybrid.finalize_indexing();

        assert_eq!(hybrid.doc_texts.len(), 2);
        assert_eq!(hybrid.bm25.doc_lengths.len(), 2);
    }

    #[test]
    fn test_normalize_minmax() {
        let mut scores = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        normalize_scores(&mut scores, NormalizationStrategy::MinMax);

        assert!((scores[0] - 0.0).abs() < 0.01);
        assert!((scores[4] - 1.0).abs() < 0.01);
        assert!((scores[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bm25_candidate_retrieval() {
        let mut bm25 = BM25::new(1.5, 0.75);

        bm25.index_document("doc1".to_string(), "rust programming");
        bm25.index_document("doc2".to_string(), "python programming");
        bm25.index_document("doc3".to_string(), "java development");
        bm25.build_idf();

        let candidates = bm25.get_candidate_docs("rust programming");
        assert!(candidates.contains(&"doc1".to_string()));
        assert!(candidates.contains(&"doc2".to_string())); // Contains "programming"
        assert!(!candidates.contains(&"doc3".to_string()));
    }
}

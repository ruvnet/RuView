//! Hybrid search combining semantic and keyword approaches.

use crate::error::Result;
use crate::storage::{SearchResult, VectorStore};
use std::collections::HashMap;
use uuid::Uuid;

/// Hybrid search that combines semantic vector similarity with keyword
/// matching using a configurable weight parameter.
pub struct HybridSearch {
    /// Weight for semantic search (1.0 = pure semantic, 0.0 = pure keyword).
    semantic_weight: f32,
}

impl HybridSearch {
    /// Create a new hybrid search with the given semantic weight.
    ///
    /// The weight controls the balance between semantic (vector) and
    /// keyword (text match) scores. A value of 0.7 means 70% semantic
    /// and 30% keyword.
    pub fn new(semantic_weight: f32) -> Self {
        Self {
            semantic_weight: semantic_weight.clamp(0.0, 1.0),
        }
    }

    /// Perform a hybrid search combining semantic and keyword results.
    ///
    /// The `query` is used for keyword matching against stored text content.
    /// The `embedding` is used for semantic similarity scoring.
    pub fn search(
        &self,
        store: &VectorStore,
        query: &str,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        // Get semantic results (more candidates than needed for merging)
        let candidate_k = (k * 3).max(20).min(store.len());
        let semantic_results = store.search(embedding, candidate_k)?;

        // Build a combined score map
        let mut scores: HashMap<Uuid, (f32, f32, serde_json::Value)> = HashMap::new();

        // Add semantic scores
        for result in &semantic_results {
            scores
                .entry(result.id)
                .or_insert((0.0, 0.0, result.metadata.clone()))
                .0 = result.score;
        }

        // Compute keyword scores for all candidates
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        for result in &semantic_results {
            let text = result
                .metadata
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text_lower = text.to_lowercase();

            let keyword_score = compute_keyword_score(&query_terms, &text_lower);

            if let Some(entry) = scores.get_mut(&result.id) {
                entry.1 = keyword_score;
            }
        }

        // Combine scores using weighted sum
        let keyword_weight = 1.0 - self.semantic_weight;
        let mut combined: Vec<SearchResult> = scores
            .into_iter()
            .map(|(id, (sem_score, kw_score, metadata))| {
                let combined_score = self.semantic_weight * sem_score + keyword_weight * kw_score;
                SearchResult {
                    id,
                    score: combined_score,
                    metadata,
                }
            })
            .collect();

        // Sort by combined score descending
        combined.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        combined.truncate(k);

        Ok(combined)
    }

    /// Return the configured semantic weight.
    pub fn semantic_weight(&self) -> f32 {
        self.semantic_weight
    }
}

/// Compute a simple keyword match score based on term overlap.
///
/// Returns a value between 0.0 and 1.0 representing the fraction
/// of query terms found in the text.
fn compute_keyword_score(query_terms: &[&str], text_lower: &str) -> f32 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let matches = query_terms
        .iter()
        .filter(|term| text_lower.contains(*term))
        .count();
    matches as f32 / query_terms.len() as f32
}

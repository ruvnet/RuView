//! Enhanced search orchestrator.
//!
//! Combines query routing, attention-based re-ranking, and quantum-inspired
//! diversity selection into a single search pipeline:
//!
//! ```text
//! Route -> Search (3x k candidates) -> Rerank (attention) -> Diversity (quantum) -> Return
//! ```

use crate::error::Result;
use crate::quantum::QuantumSearch;
use crate::search::reranker::AttentionReranker;
use crate::search::router::QueryRouter;
use crate::storage::vector_store::{SearchResult, VectorStore};

/// Orchestrates a full search pipeline: routing, candidate retrieval,
/// attention re-ranking, and quantum diversity selection.
pub struct EnhancedSearch {
    router: QueryRouter,
    reranker: Option<AttentionReranker>,
    quantum: Option<QuantumSearch>,
}

impl EnhancedSearch {
    /// Create a new enhanced search with all components wired.
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension used to configure the attention reranker.
    pub fn new(dim: usize) -> Self {
        Self {
            router: QueryRouter::new(),
            reranker: Some(AttentionReranker::new(dim, 4)),
            quantum: Some(QuantumSearch::new()),
        }
    }

    /// Create an enhanced search with only the router (no reranking or diversity).
    pub fn router_only() -> Self {
        Self {
            router: QueryRouter::new(),
            reranker: None,
            quantum: None,
        }
    }

    /// Return a reference to the query router.
    pub fn router(&self) -> &QueryRouter {
        &self.router
    }

    /// Search the vector store with routing, re-ranking, and diversity selection.
    ///
    /// The pipeline:
    /// 1. Route the query to determine the search strategy.
    /// 2. Fetch `3 * k` candidates from the store to give the reranker headroom.
    /// 3. If a reranker is available, re-rank candidates using attention scores.
    /// 4. If quantum diversity selection is available, select the final `k`
    ///    results with maximum diversity.
    /// 5. Return the final results.
    pub fn search(
        &self,
        query: &str,
        query_embedding: &[f32],
        store: &VectorStore,
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        // Step 1: Route the query (informational -- we always search the
        // vector store for now, but the route is available for future use).
        let _route = self.router.route(query);

        // Step 2: Fetch candidates with headroom for reranking.
        let candidate_k = (k * 3).max(10).min(store.len().max(1));
        let candidates = store.search(query_embedding, candidate_k)?;

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Step 3: Re-rank with attention if available.
        let results = if let Some(ref reranker) = self.reranker {
            // Build the tuples the reranker expects: (id_string, score, embedding).
            let reranker_input: Vec<(String, f32, Vec<f32>)> = candidates
                .iter()
                .map(|sr| {
                    // Retrieve the stored embedding for this result.
                    let embedding = store
                        .get(&sr.id)
                        .map(|stored| stored.vector.clone())
                        .unwrap_or_else(|| vec![0.0; query_embedding.len()]);
                    (sr.id.to_string(), sr.score, embedding)
                })
                .collect();

            // The reranker returns more than k so quantum diversity can choose.
            let rerank_k = if self.quantum.is_some() {
                (k * 2).min(reranker_input.len())
            } else {
                k
            };
            let reranked = reranker.rerank(query_embedding, &reranker_input, rerank_k);

            // Step 4: Diversity selection if available.
            let final_scored = if let Some(ref quantum) = self.quantum {
                quantum.diversity_select(&reranked, k)
            } else {
                let mut r = reranked;
                r.truncate(k);
                r
            };

            // Map back to SearchResult by looking up metadata from candidates.
            final_scored
                .into_iter()
                .filter_map(|(id_str, score)| {
                    // Parse the UUID back.
                    let uid: uuid::Uuid = id_str.parse().ok()?;
                    // Find the original candidate to retrieve metadata.
                    let original = candidates.iter().find(|c| c.id == uid)?;
                    Some(SearchResult {
                        id: uid,
                        score,
                        metadata: original.metadata.clone(),
                    })
                })
                .collect()
        } else {
            // No reranker -- just truncate.
            candidates.into_iter().take(k).collect()
        };

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;
    use crate::config::StorageConfig;
    use crate::storage::embedding::EmbeddingEngine;

    #[test]
    fn test_enhanced_search_empty_store() {
        let config = StorageConfig::default();
        let store = VectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);
        let es = EnhancedSearch::new(384);

        let query_emb = engine.embed("test query");
        let results = es.search("test query", &query_emb, &store, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_enhanced_search_returns_results() {
        let config = StorageConfig::default();
        let mut store = VectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let frames = vec![
            CapturedFrame::new_screen("Editor", "code.rs", "implementing vector search in Rust", 0),
            CapturedFrame::new_screen("Browser", "docs", "Rust vector database documentation", 0),
            CapturedFrame::new_audio("Mic", "discussing Python machine learning", None),
        ];

        for frame in &frames {
            let emb = engine.embed(frame.text_content());
            store.insert(frame, &emb).unwrap();
        }

        let es = EnhancedSearch::new(384);
        let query_emb = engine.embed("vector search Rust");
        let results = es
            .search("vector search Rust", &query_emb, &store, 2)
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }

    #[test]
    fn test_enhanced_search_router_only() {
        let config = StorageConfig::default();
        let mut store = VectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let frame = CapturedFrame::new_screen("App", "Win", "test content", 0);
        let emb = engine.embed(frame.text_content());
        store.insert(&frame, &emb).unwrap();

        let es = EnhancedSearch::router_only();
        let query_emb = engine.embed("test content");
        let results = es.search("test content", &query_emb, &store, 5).unwrap();

        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_enhanced_search_respects_k() {
        let config = StorageConfig::default();
        let mut store = VectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        for i in 0..10 {
            let frame = CapturedFrame::new_screen("App", "Win", &format!("content {}", i), 0);
            let emb = engine.embed(frame.text_content());
            store.insert(&frame, &emb).unwrap();
        }

        let es = EnhancedSearch::new(384);
        let query_emb = engine.embed("content");
        let results = es.search("content", &query_emb, &store, 3).unwrap();

        assert!(
            results.len() <= 3,
            "Should return at most k=3 results, got {}",
            results.len()
        );
    }
}

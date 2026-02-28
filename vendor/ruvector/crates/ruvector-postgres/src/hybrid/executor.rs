//! Hybrid Query Executor
//!
//! Executes vector and keyword search branches, optionally in parallel,
//! and fuses results using the configured algorithm.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::bm25::{tokenize_query, BM25Scorer, CorpusStats, Document, TermFrequencies};
use super::fusion::{
    fuse_results, learned_fusion, DocId, FusedResult, FusionConfig, FusionMethod, FusionModel,
};

/// Hybrid search query
#[derive(Debug, Clone)]
pub struct HybridQuery {
    /// Query text for keyword search
    pub text: String,
    /// Query embedding for vector search
    pub embedding: Vec<f32>,
    /// Number of final results to return
    pub k: usize,
    /// Number of results to prefetch from each branch
    pub prefetch_k: usize,
    /// Fusion configuration
    pub fusion_config: FusionConfig,
    /// Optional filter expression
    pub filter: Option<String>,
}

impl HybridQuery {
    /// Create a new hybrid query
    pub fn new(text: String, embedding: Vec<f32>, k: usize) -> Self {
        Self {
            text,
            embedding,
            k,
            prefetch_k: k * 10, // Default prefetch 10x final k
            fusion_config: FusionConfig::default(),
            filter: None,
        }
    }

    /// Set fusion method
    pub fn with_fusion(mut self, method: FusionMethod) -> Self {
        self.fusion_config.method = method;
        self
    }

    /// Set fusion alpha (for linear fusion)
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.fusion_config.alpha = alpha;
        self
    }

    /// Set RRF k constant
    pub fn with_rrf_k(mut self, rrf_k: usize) -> Self {
        self.fusion_config.rrf_k = rrf_k;
        self
    }

    /// Set prefetch size
    pub fn with_prefetch(mut self, prefetch_k: usize) -> Self {
        self.prefetch_k = prefetch_k;
        self
    }

    /// Set filter expression
    pub fn with_filter(mut self, filter: String) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Execution strategy for hybrid search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HybridStrategy {
    /// Execute both branches fully
    Full,
    /// Pre-filter using keyword/metadata, then vector search on subset
    PreFilter,
    /// Execute hybrid search, then apply filter
    PostFilter,
    /// Vector search only (degraded mode)
    VectorOnly,
    /// Keyword search only (degraded mode)
    KeywordOnly,
}

impl Default for HybridStrategy {
    fn default() -> Self {
        HybridStrategy::Full
    }
}

/// Choose execution strategy based on query characteristics
pub fn choose_strategy(
    filter_selectivity: Option<f32>,
    corpus_size: u64,
    has_filter: bool,
) -> HybridStrategy {
    if !has_filter {
        return HybridStrategy::Full;
    }

    match filter_selectivity {
        Some(sel) if sel < 0.01 => {
            // Very selective filter: pre-filter first
            HybridStrategy::PreFilter
        }
        Some(sel) if sel < 0.1 && corpus_size > 1_000_000 => {
            // Moderately selective on large corpus: post-filter
            HybridStrategy::PostFilter
        }
        _ => HybridStrategy::Full,
    }
}

/// Result from a single search branch
#[derive(Debug, Clone)]
pub struct BranchResults {
    /// Document IDs and scores
    pub results: Vec<(DocId, f32)>,
    /// Execution time in milliseconds
    pub latency_ms: f64,
    /// Number of candidates evaluated
    pub candidates_evaluated: usize,
}

impl BranchResults {
    /// Create empty results
    pub fn empty() -> Self {
        Self {
            results: Vec::new(),
            latency_ms: 0.0,
            candidates_evaluated: 0,
        }
    }
}

/// Hybrid search result with detailed scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridResult {
    /// Document ID
    pub id: DocId,
    /// Final hybrid score
    pub hybrid_score: f32,
    /// Vector similarity score (1 - distance for cosine)
    pub vector_score: Option<f32>,
    /// BM25 keyword score
    pub keyword_score: Option<f32>,
    /// Rank in vector results (None if not present)
    pub vector_rank: Option<usize>,
    /// Rank in keyword results (None if not present)
    pub keyword_rank: Option<usize>,
}

impl From<FusedResult> for HybridResult {
    fn from(fused: FusedResult) -> Self {
        Self {
            id: fused.doc_id,
            hybrid_score: fused.hybrid_score,
            vector_score: fused.vector_score,
            keyword_score: fused.keyword_score,
            vector_rank: None,
            keyword_rank: None,
        }
    }
}

/// Execution statistics for hybrid search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total execution time in milliseconds
    pub total_latency_ms: f64,
    /// Vector branch latency
    pub vector_latency_ms: f64,
    /// Keyword branch latency
    pub keyword_latency_ms: f64,
    /// Fusion latency
    pub fusion_latency_ms: f64,
    /// Strategy used
    pub strategy: String,
    /// Number of vector candidates
    pub vector_candidates: usize,
    /// Number of keyword candidates
    pub keyword_candidates: usize,
    /// Final result count
    pub result_count: usize,
}

/// Hybrid Query Executor
///
/// Coordinates vector and keyword search branches, handles parallel execution,
/// and manages score fusion.
pub struct HybridExecutor {
    /// BM25 scorer for keyword scoring
    bm25_scorer: BM25Scorer,
    /// Fusion model for learned fusion
    fusion_model: FusionModel,
    /// Whether to use parallel execution
    parallel_enabled: bool,
    /// Default prefetch multiplier
    prefetch_multiplier: usize,
}

impl HybridExecutor {
    /// Create a new hybrid executor
    pub fn new(corpus_stats: CorpusStats) -> Self {
        Self {
            bm25_scorer: BM25Scorer::new(corpus_stats),
            fusion_model: FusionModel::default(),
            parallel_enabled: true,
            prefetch_multiplier: 10,
        }
    }

    /// Update corpus statistics
    pub fn update_corpus_stats(&mut self, stats: CorpusStats) {
        self.bm25_scorer.update_corpus_stats(stats);
    }

    /// Set whether to use parallel execution
    pub fn set_parallel(&mut self, enabled: bool) {
        self.parallel_enabled = enabled;
    }

    /// Set prefetch multiplier
    pub fn set_prefetch_multiplier(&mut self, multiplier: usize) {
        self.prefetch_multiplier = multiplier;
    }

    /// Execute hybrid search
    ///
    /// This is the main entry point for hybrid search. In the PostgreSQL extension,
    /// this would call into the actual vector index and tsvector search.
    pub fn execute(
        &self,
        query: &HybridQuery,
        vector_search_fn: impl FnOnce(&[f32], usize) -> BranchResults,
        keyword_search_fn: impl FnOnce(&str, usize) -> BranchResults,
    ) -> (Vec<HybridResult>, ExecutionStats) {
        let start = std::time::Instant::now();

        // Execute both branches
        let (vector_results, keyword_results) = if self.parallel_enabled {
            // In async context, use tokio::join!
            // For sync, execute sequentially (rayon could parallelize)
            let v_start = std::time::Instant::now();
            let vector = vector_search_fn(&query.embedding, query.prefetch_k);
            let v_elapsed = v_start.elapsed().as_secs_f64() * 1000.0;

            let k_start = std::time::Instant::now();
            let keyword = keyword_search_fn(&query.text, query.prefetch_k);
            let k_elapsed = k_start.elapsed().as_secs_f64() * 1000.0;

            (
                BranchResults {
                    latency_ms: v_elapsed,
                    ..vector
                },
                BranchResults {
                    latency_ms: k_elapsed,
                    ..keyword
                },
            )
        } else {
            let v_start = std::time::Instant::now();
            let vector = vector_search_fn(&query.embedding, query.prefetch_k);
            let v_elapsed = v_start.elapsed().as_secs_f64() * 1000.0;

            let k_start = std::time::Instant::now();
            let keyword = keyword_search_fn(&query.text, query.prefetch_k);
            let k_elapsed = k_start.elapsed().as_secs_f64() * 1000.0;

            (
                BranchResults {
                    latency_ms: v_elapsed,
                    ..vector
                },
                BranchResults {
                    latency_ms: k_elapsed,
                    ..keyword
                },
            )
        };

        // Fuse results
        let fusion_start = std::time::Instant::now();
        let fused = self.fuse(&query, &vector_results.results, &keyword_results.results);
        let fusion_elapsed = fusion_start.elapsed().as_secs_f64() * 1000.0;

        // Add rank information
        let vector_ranks: HashMap<DocId, usize> = vector_results
            .results
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();

        let keyword_ranks: HashMap<DocId, usize> = keyword_results
            .results
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();

        let results: Vec<HybridResult> = fused
            .into_iter()
            .take(query.k)
            .map(|f| {
                let mut result = HybridResult::from(f);
                result.vector_rank = vector_ranks.get(&result.id).copied();
                result.keyword_rank = keyword_ranks.get(&result.id).copied();
                result
            })
            .collect();

        let total_elapsed = start.elapsed().as_secs_f64() * 1000.0;

        let stats = ExecutionStats {
            total_latency_ms: total_elapsed,
            vector_latency_ms: vector_results.latency_ms,
            keyword_latency_ms: keyword_results.latency_ms,
            fusion_latency_ms: fusion_elapsed,
            strategy: "full".to_string(),
            vector_candidates: vector_results.candidates_evaluated,
            keyword_candidates: keyword_results.candidates_evaluated,
            result_count: results.len(),
        };

        (results, stats)
    }

    /// Fuse vector and keyword results
    fn fuse(
        &self,
        query: &HybridQuery,
        vector_results: &[(DocId, f32)],
        keyword_results: &[(DocId, f32)],
    ) -> Vec<FusedResult> {
        match query.fusion_config.method {
            FusionMethod::Learned => {
                // Use learned fusion with query features
                let query_terms = tokenize_query(&query.text);
                let avg_idf = self.compute_avg_idf(&query_terms);

                learned_fusion(
                    &query.embedding,
                    &query_terms,
                    vector_results,
                    keyword_results,
                    &self.fusion_model,
                    avg_idf,
                    query.prefetch_k,
                )
            }
            _ => {
                // Use standard fusion
                fuse_results(
                    vector_results,
                    keyword_results,
                    &query.fusion_config,
                    query.prefetch_k,
                )
            }
        }
    }

    /// Compute average IDF for query terms
    fn compute_avg_idf(&self, terms: &[String]) -> f32 {
        if terms.is_empty() {
            return 0.0;
        }

        let total_idf: f32 = terms.iter().map(|t| self.bm25_scorer.idf(t)).sum();
        total_idf / terms.len() as f32
    }

    /// Score documents using BM25
    pub fn bm25_score(&self, term_freqs: &TermFrequencies, query_terms: &[String]) -> f32 {
        let doc = Document::new(term_freqs);
        self.bm25_scorer.score(&doc, query_terms)
    }

    /// Set document frequency for a term (for BM25 IDF calculation)
    pub fn set_doc_freq(&self, term: &str, doc_freq: u64) {
        self.bm25_scorer.set_doc_freq(term, doc_freq);
    }

    /// Get current corpus statistics
    pub fn corpus_stats(&self) -> &CorpusStats {
        self.bm25_scorer.corpus_stats()
    }
}

/// Async hybrid execution using tokio
#[cfg(feature = "tokio")]
pub mod async_executor {
    use super::*;
    use std::future::Future;

    /// Execute hybrid search with parallel branches
    pub async fn parallel_hybrid<VF, KF, VFut, KFut>(
        query: &HybridQuery,
        vector_search: VF,
        keyword_search: KF,
        fusion_config: &FusionConfig,
    ) -> Vec<FusedResult>
    where
        VF: FnOnce(&[f32], usize) -> VFut,
        KF: FnOnce(&str, usize) -> KFut,
        VFut: Future<Output = BranchResults>,
        KFut: Future<Output = BranchResults>,
    {
        let (vector_results, keyword_results) = tokio::join!(
            vector_search(&query.embedding, query.prefetch_k),
            keyword_search(&query.text, query.prefetch_k),
        );

        fuse_results(
            &vector_results.results,
            &keyword_results.results,
            fusion_config,
            query.k,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_vector_search(_embedding: &[f32], k: usize) -> BranchResults {
        BranchResults {
            results: (0..k.min(5))
                .map(|i| (i as DocId + 1, 0.1 * (i + 1) as f32))
                .collect(),
            latency_ms: 1.0,
            candidates_evaluated: 100,
        }
    }

    fn mock_keyword_search(_text: &str, k: usize) -> BranchResults {
        BranchResults {
            results: (0..k.min(5))
                .map(|i| ((i as DocId + 3), 10.0 - i as f32))
                .collect(),
            latency_ms: 0.5,
            candidates_evaluated: 50,
        }
    }

    #[test]
    fn test_hybrid_query_builder() {
        let query = HybridQuery::new("test query".into(), vec![0.1, 0.2, 0.3], 10)
            .with_fusion(FusionMethod::Linear)
            .with_alpha(0.7)
            .with_prefetch(100)
            .with_filter("category = 'docs'".into());

        assert_eq!(query.k, 10);
        assert_eq!(query.prefetch_k, 100);
        assert_eq!(query.fusion_config.method, FusionMethod::Linear);
        assert!((query.fusion_config.alpha - 0.7).abs() < 0.01);
        assert!(query.filter.is_some());
    }

    #[test]
    fn test_hybrid_executor() {
        let stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 1000,
            total_terms: 100000,
            last_update: 0,
        };

        let executor = HybridExecutor::new(stats);

        let query = HybridQuery::new("database query".into(), vec![0.1; 128], 5);

        let (results, exec_stats) =
            executor.execute(&query, mock_vector_search, mock_keyword_search);

        assert!(!results.is_empty());
        assert!(results.len() <= 5);
        assert!(exec_stats.total_latency_ms > 0.0);
    }

    #[test]
    fn test_strategy_selection() {
        // No filter -> Full
        assert_eq!(choose_strategy(None, 10000, false), HybridStrategy::Full);

        // Very selective filter -> PreFilter
        assert_eq!(
            choose_strategy(Some(0.005), 1000000, true),
            HybridStrategy::PreFilter
        );

        // Moderate selectivity, large corpus -> PostFilter
        assert_eq!(
            choose_strategy(Some(0.05), 5000000, true),
            HybridStrategy::PostFilter
        );
    }

    #[test]
    fn test_execution_stats() {
        let stats = CorpusStats::default();
        let executor = HybridExecutor::new(stats);

        let query = HybridQuery::new("test".into(), vec![0.1; 16], 5);

        let (_, exec_stats) = executor.execute(&query, mock_vector_search, mock_keyword_search);

        assert!(exec_stats.vector_latency_ms >= 0.0);
        assert!(exec_stats.keyword_latency_ms >= 0.0);
        assert!(exec_stats.fusion_latency_ms >= 0.0);
        assert!(exec_stats.total_latency_ms >= exec_stats.fusion_latency_ms);
    }
}

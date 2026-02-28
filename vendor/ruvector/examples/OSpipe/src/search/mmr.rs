//! Maximal Marginal Relevance (MMR) re-ranking.
//!
//! MMR balances relevance to the query with diversity among selected
//! results, controlled by a `lambda` parameter:
//! - `lambda = 1.0` produces pure relevance ranking (identical to cosine).
//! - `lambda = 0.0` maximises diversity among selected results.
//!
//! The `lambda` value is sourced from [`SearchConfig::mmr_lambda`](crate::config::SearchConfig).

/// Re-ranks search results using Maximal Marginal Relevance.
pub struct MmrReranker {
    /// Trade-off between relevance and diversity.
    /// 1.0 = pure relevance, 0.0 = pure diversity.
    lambda: f32,
}

impl MmrReranker {
    /// Create a new MMR reranker with the given lambda.
    pub fn new(lambda: f32) -> Self {
        Self { lambda }
    }

    /// Re-rank results using MMR to balance relevance and diversity.
    ///
    /// # Arguments
    ///
    /// * `query_embedding` - The query vector.
    /// * `results` - Candidate results as `(id, score, embedding)` tuples.
    /// * `k` - Maximum number of results to return.
    ///
    /// # Returns
    ///
    /// A `Vec` of `(id, mmr_score)` pairs in MMR-selected order,
    /// truncated to at most `k` entries.
    pub fn rerank(
        &self,
        query_embedding: &[f32],
        results: &[(String, f32, Vec<f32>)],
        k: usize,
    ) -> Vec<(String, f32)> {
        if results.is_empty() {
            return Vec::new();
        }

        let n = results.len().min(k);

        // Precompute similarities between the query and each document.
        let query_sims: Vec<f32> = results
            .iter()
            .map(|(_, _, emb)| cosine_sim(query_embedding, emb))
            .collect();

        let mut selected: Vec<usize> = Vec::with_capacity(n);
        let mut selected_set = vec![false; results.len()];
        let mut output: Vec<(String, f32)> = Vec::with_capacity(n);

        for _ in 0..n {
            let mut best_idx = None;
            let mut best_mmr = f32::NEG_INFINITY;

            for (i, _) in results.iter().enumerate() {
                if selected_set[i] {
                    continue;
                }

                let relevance = query_sims[i];

                // Max similarity to any already-selected document.
                let max_sim_to_selected = if selected.is_empty() {
                    0.0
                } else {
                    selected
                        .iter()
                        .map(|&j| cosine_sim(&results[i].2, &results[j].2))
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                let mmr = self.lambda * relevance - (1.0 - self.lambda) * max_sim_to_selected;

                if mmr > best_mmr {
                    best_mmr = mmr;
                    best_idx = Some(i);
                }
            }

            if let Some(idx) = best_idx {
                selected.push(idx);
                selected_set[idx] = true;
                output.push((results[idx].0.clone(), best_mmr));
            } else {
                break;
            }
        }

        output
    }
}

/// Cosine similarity between two vectors.
///
/// Returns 0.0 when either vector has zero magnitude.
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let mut dot: f32 = 0.0;
    let mut mag_a: f32 = 0.0;
    let mut mag_b: f32 = 0.0;

    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmr_empty_results() {
        let mmr = MmrReranker::new(0.5);
        let result = mmr.rerank(&[1.0, 0.0], &[], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mmr_single_result() {
        let mmr = MmrReranker::new(0.5);
        let results = vec![("a".to_string(), 0.9, vec![1.0, 0.0])];
        let ranked = mmr.rerank(&[1.0, 0.0], &results, 5);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].0, "a");
    }

    #[test]
    fn test_mmr_pure_relevance() {
        // lambda=1.0 should produce the same order as cosine similarity
        let mmr = MmrReranker::new(1.0);
        let query = vec![1.0, 0.0, 0.0];
        let results = vec![
            ("best".to_string(), 0.9, vec![1.0, 0.0, 0.0]),
            ("mid".to_string(), 0.7, vec![0.7, 0.7, 0.0]),
            ("worst".to_string(), 0.3, vec![0.0, 0.0, 1.0]),
        ];

        let ranked = mmr.rerank(&query, &results, 3);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].0, "best");
    }

    #[test]
    fn test_mmr_promotes_diversity() {
        // With lambda < 1.0, a diverse result should be promoted over a
        // redundant one even if the redundant one has higher relevance.
        let mmr = MmrReranker::new(0.3);
        let query = vec![1.0, 0.0, 0.0, 0.0];

        // Two results very similar to each other and the query,
        // one result orthogonal but moderately relevant.
        let results = vec![
            ("a".to_string(), 0.95, vec![1.0, 0.0, 0.0, 0.0]),
            ("a_clone".to_string(), 0.90, vec![0.99, 0.01, 0.0, 0.0]),
            ("diverse".to_string(), 0.60, vec![0.0, 1.0, 0.0, 0.0]),
        ];

        let ranked = mmr.rerank(&query, &results, 3);
        assert_eq!(ranked.len(), 3);

        // "a" should be first (highest relevance)
        assert_eq!(ranked[0].0, "a");

        // "diverse" should be second because "a_clone" is too similar to "a"
        assert_eq!(
            ranked[1].0, "diverse",
            "MMR should promote diverse result over near-duplicate"
        );
    }

    #[test]
    fn test_mmr_respects_top_k() {
        let mmr = MmrReranker::new(0.5);
        let query = vec![1.0, 0.0];
        let results = vec![
            ("a".to_string(), 0.9, vec![1.0, 0.0]),
            ("b".to_string(), 0.8, vec![0.0, 1.0]),
            ("c".to_string(), 0.7, vec![0.5, 0.5]),
        ];

        let ranked = mmr.rerank(&query, &results, 2);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn test_cosine_sim_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_sim(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_sim_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_sim(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_sim_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_sim(&a, &b), 0.0);
    }
}

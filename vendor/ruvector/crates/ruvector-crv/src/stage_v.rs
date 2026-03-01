//! Stage V: Interrogation via Differentiable Search with Soft Attention
//!
//! CRV Stage V involves probing the signal line by asking targeted questions
//! about specific aspects of the target, then cross-referencing results
//! across all accumulated data from Stages I-IV.
//!
//! # Architecture
//!
//! Uses `ruvector_gnn::search::differentiable_search` to find the most
//! relevant data entries for each probe query, with soft attention weights
//! providing a continuous similarity measure rather than hard thresholds.
//! This enables gradient-based refinement of probe queries.

use crate::error::{CrvError, CrvResult};
use crate::types::{CrossReference, CrvConfig, SignalLineProbe, StageVData};
use ruvector_gnn::search::{cosine_similarity, differentiable_search};

/// Stage V interrogation engine using differentiable search.
#[derive(Debug, Clone)]
pub struct StageVEngine {
    /// Embedding dimensionality.
    dim: usize,
    /// Temperature for differentiable search softmax.
    temperature: f32,
}

impl StageVEngine {
    /// Create a new Stage V engine.
    pub fn new(config: &CrvConfig) -> Self {
        Self {
            dim: config.dimensions,
            temperature: config.search_temperature,
        }
    }

    /// Probe the accumulated session embeddings with a query.
    ///
    /// Performs differentiable search over the given candidate embeddings,
    /// returning soft attention weights and top-k candidates.
    pub fn probe(
        &self,
        query_embedding: &[f32],
        candidates: &[Vec<f32>],
        k: usize,
    ) -> CrvResult<SignalLineProbe> {
        if candidates.is_empty() {
            return Err(CrvError::EmptyInput(
                "No candidates for probing".to_string(),
            ));
        }

        let (top_candidates, attention_weights) =
            differentiable_search(query_embedding, candidates, k, self.temperature);

        Ok(SignalLineProbe {
            query: String::new(), // Caller sets the text
            target_stage: 0,      // Caller sets the stage
            attention_weights,
            top_candidates,
        })
    }

    /// Cross-reference entries across stages to find correlations.
    ///
    /// For each entry in `from_entries`, finds the most similar entries
    /// in `to_entries` using cosine similarity, producing cross-references
    /// above the given threshold.
    pub fn cross_reference(
        &self,
        from_stage: u8,
        from_entries: &[Vec<f32>],
        to_stage: u8,
        to_entries: &[Vec<f32>],
        threshold: f32,
    ) -> Vec<CrossReference> {
        let mut refs = Vec::new();

        for (from_idx, from_emb) in from_entries.iter().enumerate() {
            for (to_idx, to_emb) in to_entries.iter().enumerate() {
                if from_emb.len() == to_emb.len() {
                    let score = cosine_similarity(from_emb, to_emb);
                    if score >= threshold {
                        refs.push(CrossReference {
                            from_stage,
                            from_entry: from_idx,
                            to_stage,
                            to_entry: to_idx,
                            score,
                        });
                    }
                }
            }
        }

        // Sort by score descending
        refs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        refs
    }

    /// Encode Stage V data into a combined interrogation embedding.
    ///
    /// Aggregates the attention weights from all probes to produce
    /// a unified view of which aspects of the target were most
    /// responsive to interrogation.
    pub fn encode(&self, data: &StageVData, all_embeddings: &[Vec<f32>]) -> CrvResult<Vec<f32>> {
        if data.probes.is_empty() {
            return Err(CrvError::EmptyInput(
                "No probes in Stage V data".to_string(),
            ));
        }

        let mut embedding = vec![0.0f32; self.dim];

        // Weight each candidate embedding by its attention weight across all probes
        for probe in &data.probes {
            for (&candidate_idx, &weight) in probe
                .top_candidates
                .iter()
                .zip(probe.attention_weights.iter())
            {
                if candidate_idx < all_embeddings.len() {
                    let emb = &all_embeddings[candidate_idx];
                    for (i, &v) in emb.iter().enumerate() {
                        if i < self.dim {
                            embedding[i] += v * weight;
                        }
                    }
                }
            }
        }

        // Normalize by number of probes
        let num_probes = data.probes.len() as f32;
        for v in &mut embedding {
            *v /= num_probes;
        }

        Ok(embedding)
    }

    /// Compute the interrogation signal strength for a given embedding.
    ///
    /// Higher values indicate more responsive signal line data.
    pub fn signal_strength(&self, embedding: &[f32]) -> f32 {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 8,
            search_temperature: 1.0,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_engine_creation() {
        let config = test_config();
        let engine = StageVEngine::new(&config);
        assert_eq!(engine.dim, 8);
    }

    #[test]
    fn test_probe() {
        let config = test_config();
        let engine = StageVEngine::new(&config);

        let query = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let candidates = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // exact match
            vec![0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // partial
            vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // orthogonal
        ];

        let probe = engine.probe(&query, &candidates, 2).unwrap();
        assert_eq!(probe.top_candidates.len(), 2);
        assert_eq!(probe.attention_weights.len(), 2);
        // Best match should be first
        assert_eq!(probe.top_candidates[0], 0);
    }

    #[test]
    fn test_cross_reference() {
        let config = test_config();
        let engine = StageVEngine::new(&config);

        let from = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let to = vec![
            vec![0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // similar to from[0]
            vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], // different
        ];

        let refs = engine.cross_reference(1, &from, 2, &to, 0.5);
        assert!(!refs.is_empty());
        assert_eq!(refs[0].from_stage, 1);
        assert_eq!(refs[0].to_stage, 2);
        assert!(refs[0].score > 0.5);
    }

    #[test]
    fn test_empty_probe() {
        let config = test_config();
        let engine = StageVEngine::new(&config);

        let query = vec![1.0; 8];
        let candidates: Vec<Vec<f32>> = vec![];

        assert!(engine.probe(&query, &candidates, 5).is_err());
    }
}

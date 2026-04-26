//! Temporal sliding-window embeddings for brain graph sequences.
//!
//! Embeds a time series of brain graphs into trajectory vectors by combining
//! each graph's embedding with an exponentially-weighted average of past embeddings.

use ruv_neural_core::embedding::{EmbeddingTrajectory, NeuralEmbedding};
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::graph::{BrainGraph, BrainGraphSequence};
use ruv_neural_core::traits::EmbeddingGenerator;

use crate::default_metadata;

/// Temporal embedder that enriches each graph embedding with historical context.
pub struct TemporalEmbedder {
    /// Base embedder for individual graphs.
    base_embedder: Box<dyn EmbeddingGenerator>,
    /// Number of past embeddings to consider in the context window.
    window_size: usize,
    /// Exponential decay factor for weighting past embeddings (0 < decay <= 1).
    decay: f64,
}

impl TemporalEmbedder {
    /// Create a new temporal embedder.
    ///
    /// - `base`: the embedding generator for individual graphs
    /// - `window`: how many past embeddings to incorporate
    pub fn new(base: Box<dyn EmbeddingGenerator>, window: usize) -> Self {
        Self {
            base_embedder: base,
            window_size: window,
            decay: 0.8,
        }
    }

    /// Set the exponential decay factor.
    pub fn with_decay(mut self, decay: f64) -> Self {
        self.decay = decay.clamp(0.01, 1.0);
        self
    }

    /// Embed a full sequence of graphs into a trajectory.
    pub fn embed_sequence(&self, sequence: &BrainGraphSequence) -> Result<EmbeddingTrajectory> {
        if sequence.is_empty() {
            return Err(RuvNeuralError::Embedding(
                "Cannot embed empty graph sequence".into(),
            ));
        }

        let mut history: Vec<NeuralEmbedding> = Vec::new();
        let mut embeddings = Vec::with_capacity(sequence.graphs.len());
        let mut timestamps = Vec::with_capacity(sequence.graphs.len());

        for graph in &sequence.graphs {
            let emb = self.embed_with_context(graph, &history)?;
            timestamps.push(graph.timestamp);
            history.push(self.base_embedder.embed(graph)?);
            embeddings.push(emb);
        }

        Ok(EmbeddingTrajectory {
            embeddings,
            timestamps,
        })
    }

    /// Embed a single graph with temporal context from past embeddings.
    ///
    /// The output concatenates:
    /// 1. The current graph's base embedding
    /// 2. An exponentially-weighted average of past embeddings (zero-padded if no history)
    pub fn embed_with_context(
        &self,
        graph: &BrainGraph,
        history: &[NeuralEmbedding],
    ) -> Result<NeuralEmbedding> {
        let current = self.base_embedder.embed(graph)?;
        let base_dim = current.dimension;

        let context = self.compute_context(history, base_dim);

        let mut values = Vec::with_capacity(base_dim * 2);
        values.extend_from_slice(&current.vector);
        values.extend_from_slice(&context);

        let meta = default_metadata("temporal", graph.atlas);
        NeuralEmbedding::new(values, graph.timestamp, meta)
    }

    /// Compute the exponentially-weighted context vector from history.
    fn compute_context(&self, history: &[NeuralEmbedding], dim: usize) -> Vec<f64> {
        if history.is_empty() {
            return vec![0.0; dim];
        }

        let window_start = if history.len() > self.window_size {
            history.len() - self.window_size
        } else {
            0
        };
        let window = &history[window_start..];

        let mut context = vec![0.0; dim];
        let mut total_weight = 0.0;

        for (i, emb) in window.iter().rev().enumerate() {
            let w = self.decay.powi(i as i32);
            total_weight += w;
            let usable_dim = dim.min(emb.dimension);
            for j in 0..usable_dim {
                context[j] += w * emb.vector[j];
            }
        }

        if total_weight > 1e-12 {
            for v in &mut context {
                *v /= total_weight;
            }
        }

        context
    }

    /// Output dimension: base dimension * 2 (current + context).
    pub fn output_dimension(&self) -> usize {
        self.base_embedder.embedding_dim() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology_embed::TopologyEmbedder;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_graph(timestamp: f64) -> BrainGraph {
        BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.5,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp,
            window_duration_s: 0.5,
            atlas: Atlas::Custom(3),
        }
    }

    #[test]
    fn test_temporal_embed_no_history() {
        let embedder = TemporalEmbedder::new(Box::new(TopologyEmbedder::new()), 5);
        let graph = make_graph(0.0);
        let emb = embedder.embed_with_context(&graph, &[]).unwrap();

        let base_dim = TopologyEmbedder::new().embedding_dim();
        assert_eq!(emb.dimension, base_dim * 2);

        for i in base_dim..emb.dimension {
            assert!(
                emb.vector[i].abs() < 1e-12,
                "Context should be zero with no history"
            );
        }
    }

    #[test]
    fn test_temporal_embed_sequence() {
        let base = Box::new(TopologyEmbedder::new());
        let embedder = TemporalEmbedder::new(base, 3);

        let sequence = BrainGraphSequence {
            graphs: vec![make_graph(0.0), make_graph(0.5), make_graph(1.0)],
            window_step_s: 0.5,
        };

        let trajectory = embedder.embed_sequence(&sequence).unwrap();
        assert_eq!(trajectory.len(), 3);
        assert_eq!(trajectory.timestamps.len(), 3);

        let base_dim = TopologyEmbedder::new().embedding_dim();
        for i in base_dim..trajectory.embeddings[0].dimension {
            assert!(trajectory.embeddings[0].vector[i].abs() < 1e-12);
        }

        let has_nonzero = trajectory.embeddings[2].vector[base_dim..]
            .iter()
            .any(|v| v.abs() > 1e-12);
        assert!(
            has_nonzero,
            "Third embedding should have non-zero temporal context"
        );
    }

    #[test]
    fn test_temporal_empty_sequence_fails() {
        let embedder = TemporalEmbedder::new(Box::new(TopologyEmbedder::new()), 3);
        let sequence = BrainGraphSequence {
            graphs: vec![],
            window_step_s: 0.5,
        };
        assert!(embedder.embed_sequence(&sequence).is_err());
    }
}

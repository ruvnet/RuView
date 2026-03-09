//! K-Nearest Neighbor decoder for cognitive state classification.

use std::collections::HashMap;

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::topology::CognitiveState;
use ruv_neural_core::traits::StateDecoder;

/// Simple KNN decoder using stored labeled embeddings.
///
/// Classifies a query embedding by majority vote among its `k` nearest
/// neighbors in Euclidean distance.
pub struct KnnDecoder {
    labeled_embeddings: Vec<(NeuralEmbedding, CognitiveState)>,
    k: usize,
}

impl KnnDecoder {
    /// Create a new KNN decoder with the given `k` (number of neighbors).
    pub fn new(k: usize) -> Self {
        let k = if k == 0 { 1 } else { k };
        Self {
            labeled_embeddings: Vec::new(),
            k,
        }
    }

    /// Load labeled training data into the decoder.
    pub fn train(&mut self, embeddings: Vec<(NeuralEmbedding, CognitiveState)>) {
        self.labeled_embeddings = embeddings;
    }

    /// Predict the cognitive state for a query embedding using majority vote.
    ///
    /// Returns `CognitiveState::Unknown` if no training data is available.
    pub fn predict(&self, embedding: &NeuralEmbedding) -> CognitiveState {
        self.predict_with_confidence(embedding).0
    }

    /// Predict the cognitive state with a confidence score in `[0, 1]`.
    ///
    /// Confidence is the fraction of the `k` nearest neighbors that agree
    /// on the winning state.
    pub fn predict_with_confidence(&self, embedding: &NeuralEmbedding) -> (CognitiveState, f64) {
        if self.labeled_embeddings.is_empty() {
            return (CognitiveState::Unknown, 0.0);
        }

        // Compute distances to all stored embeddings.
        let mut distances: Vec<(f64, &CognitiveState)> = self
            .labeled_embeddings
            .iter()
            .filter_map(|(stored, state)| {
                let dist = euclidean_distance(&embedding.vector, &stored.vector);
                Some((dist, state))
            })
            .collect();

        // Sort by distance ascending.
        distances.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k neighbors.
        let k = self.k.min(distances.len());
        let neighbors = &distances[..k];

        // Majority vote with distance weighting.
        let mut vote_counts: HashMap<CognitiveState, f64> = HashMap::new();
        for (dist, state) in neighbors {
            // Use inverse distance weighting; add epsilon to avoid division by zero.
            let weight = 1.0 / (dist + 1e-10);
            *vote_counts.entry(**state).or_insert(0.0) += weight;
        }

        // Find the state with the highest weighted vote.
        let total_weight: f64 = vote_counts.values().sum();
        let (best_state, best_weight) = vote_counts
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((CognitiveState::Unknown, 0.0));

        let confidence = if total_weight > 0.0 {
            (best_weight / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        };

        (best_state, confidence)
    }

    /// Number of stored labeled embeddings.
    pub fn num_samples(&self) -> usize {
        self.labeled_embeddings.len()
    }
}

impl StateDecoder for KnnDecoder {
    fn decode(&self, embedding: &NeuralEmbedding) -> Result<CognitiveState> {
        if self.labeled_embeddings.is_empty() {
            return Err(RuvNeuralError::Decoder(
                "KNN decoder has no training data".into(),
            ));
        }
        Ok(self.predict(embedding))
    }

    fn decode_with_confidence(
        &self,
        embedding: &NeuralEmbedding,
    ) -> Result<(CognitiveState, f64)> {
        if self.labeled_embeddings.is_empty() {
            return Err(RuvNeuralError::Decoder(
                "KNN decoder has no training data".into(),
            ));
        }
        Ok(self.predict_with_confidence(embedding))
    }
}

/// Euclidean distance between two vectors of the same length.
///
/// If lengths differ, computes distance over the shorter prefix.
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::EmbeddingMetadata;

    fn make_embedding(vector: Vec<f64>) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            0.0,
            EmbeddingMetadata {
                subject_id: None,
                session_id: None,
                cognitive_state: None,
                source_atlas: Atlas::DesikanKilliany68,
                embedding_method: "test".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn test_knn_classifies_correctly() {
        let mut decoder = KnnDecoder::new(3);
        decoder.train(vec![
            (make_embedding(vec![1.0, 0.0, 0.0]), CognitiveState::Rest),
            (make_embedding(vec![1.1, 0.1, 0.0]), CognitiveState::Rest),
            (make_embedding(vec![0.9, 0.0, 0.1]), CognitiveState::Rest),
            (
                make_embedding(vec![0.0, 1.0, 0.0]),
                CognitiveState::Focused,
            ),
            (
                make_embedding(vec![0.1, 1.1, 0.0]),
                CognitiveState::Focused,
            ),
            (
                make_embedding(vec![0.0, 0.9, 0.1]),
                CognitiveState::Focused,
            ),
        ]);

        // Query near the Rest cluster.
        let query = make_embedding(vec![1.0, 0.05, 0.0]);
        let (state, confidence) = decoder.predict_with_confidence(&query);
        assert_eq!(state, CognitiveState::Rest);
        assert!(confidence > 0.5);

        // Query near the Focused cluster.
        let query = make_embedding(vec![0.05, 1.0, 0.0]);
        let state = decoder.predict(&query);
        assert_eq!(state, CognitiveState::Focused);
    }

    #[test]
    fn test_knn_empty_returns_unknown() {
        let decoder = KnnDecoder::new(3);
        let query = make_embedding(vec![1.0, 0.0]);
        assert_eq!(decoder.predict(&query), CognitiveState::Unknown);
    }

    #[test]
    fn test_confidence_in_range() {
        let mut decoder = KnnDecoder::new(3);
        decoder.train(vec![
            (make_embedding(vec![1.0, 0.0]), CognitiveState::Rest),
            (make_embedding(vec![0.0, 1.0]), CognitiveState::Focused),
        ]);
        let query = make_embedding(vec![0.5, 0.5]);
        let (_, confidence) = decoder.predict_with_confidence(&query);
        assert!(confidence >= 0.0 && confidence <= 1.0);
    }

    #[test]
    fn test_state_decoder_trait() {
        let mut decoder = KnnDecoder::new(1);
        decoder.train(vec![(
            make_embedding(vec![1.0, 0.0]),
            CognitiveState::MotorPlanning,
        )]);
        let query = make_embedding(vec![1.0, 0.0]);
        let result = decoder.decode(&query).unwrap();
        assert_eq!(result, CognitiveState::MotorPlanning);
    }

    #[test]
    fn test_state_decoder_empty_errors() {
        let decoder = KnnDecoder::new(3);
        let query = make_embedding(vec![1.0]);
        assert!(decoder.decode(&query).is_err());
    }
}

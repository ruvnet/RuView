//! End-to-end decoder pipeline combining multiple decoding strategies.

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::topology::{CognitiveState, TopologyMetrics};
use serde::{Deserialize, Serialize};

use crate::clinical::ClinicalScorer;
use crate::knn_decoder::KnnDecoder;
use crate::threshold_decoder::ThresholdDecoder;
use crate::transition_decoder::{StateTransition, TransitionDecoder};

/// End-to-end decoder pipeline that ensembles multiple decoding strategies.
///
/// Combines KNN, threshold, and transition decoders with configurable
/// ensemble weights, and optionally includes clinical scoring.
pub struct DecoderPipeline {
    knn: Option<KnnDecoder>,
    threshold: Option<ThresholdDecoder>,
    transition: Option<TransitionDecoder>,
    clinical: Option<ClinicalScorer>,
    /// Ensemble weights: [knn_weight, threshold_weight, transition_weight].
    ensemble_weights: [f64; 3],
}

/// Output of the decoder pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoderOutput {
    /// Decoded cognitive state (ensemble result).
    pub state: CognitiveState,
    /// Overall confidence in `[0, 1]`.
    pub confidence: f64,
    /// Detected state transition, if any.
    pub transition: Option<StateTransition>,
    /// Brain health index from clinical scorer, if configured.
    pub brain_health_index: Option<f64>,
    /// Clinical warning flags.
    pub clinical_flags: Vec<String>,
    /// Timestamp of the input data.
    pub timestamp: f64,
}

impl DecoderPipeline {
    /// Create an empty pipeline with default ensemble weights.
    pub fn new() -> Self {
        Self {
            knn: None,
            threshold: None,
            transition: None,
            clinical: None,
            ensemble_weights: [1.0, 1.0, 1.0],
        }
    }

    /// Add a KNN decoder to the pipeline.
    pub fn with_knn(mut self, k: usize) -> Self {
        self.knn = Some(KnnDecoder::new(k));
        self
    }

    /// Add a threshold decoder to the pipeline.
    pub fn with_thresholds(mut self) -> Self {
        self.threshold = Some(ThresholdDecoder::new());
        self
    }

    /// Add a transition decoder to the pipeline.
    pub fn with_transitions(mut self, window: usize) -> Self {
        self.transition = Some(TransitionDecoder::new(window));
        self
    }

    /// Add a clinical scorer to the pipeline.
    pub fn with_clinical(mut self, baseline: TopologyMetrics, std: TopologyMetrics) -> Self {
        self.clinical = Some(ClinicalScorer::new(baseline, std));
        self
    }

    /// Set custom ensemble weights for [knn, threshold, transition].
    pub fn with_weights(mut self, weights: [f64; 3]) -> Self {
        self.ensemble_weights = weights;
        self
    }

    /// Get a mutable reference to the KNN decoder (for training).
    pub fn knn_mut(&mut self) -> Option<&mut KnnDecoder> {
        self.knn.as_mut()
    }

    /// Get a mutable reference to the threshold decoder (for configuring thresholds).
    pub fn threshold_mut(&mut self) -> Option<&mut ThresholdDecoder> {
        self.threshold.as_mut()
    }

    /// Get a mutable reference to the transition decoder (for registering patterns).
    pub fn transition_mut(&mut self) -> Option<&mut TransitionDecoder> {
        self.transition.as_mut()
    }

    /// Get a mutable reference to the clinical scorer.
    pub fn clinical_mut(&mut self) -> Option<&mut ClinicalScorer> {
        self.clinical.as_mut()
    }

    /// Run the full decoding pipeline on an embedding and topology metrics.
    pub fn decode(
        &mut self,
        embedding: &NeuralEmbedding,
        metrics: &TopologyMetrics,
    ) -> DecoderOutput {
        let mut candidates: Vec<(CognitiveState, f64, f64)> = Vec::new(); // (state, confidence, weight)

        // KNN decoder.
        if let Some(ref knn) = self.knn {
            let (state, conf) = knn.predict_with_confidence(embedding);
            if state != CognitiveState::Unknown {
                candidates.push((state, conf, self.ensemble_weights[0]));
            }
        }

        // Threshold decoder.
        if let Some(ref threshold) = self.threshold {
            let (state, conf) = threshold.decode(metrics);
            if state != CognitiveState::Unknown {
                candidates.push((state, conf, self.ensemble_weights[1]));
            }
        }

        // Transition decoder.
        let transition = if let Some(ref mut trans) = self.transition {
            let result = trans.update(metrics.clone());
            if let Some(ref t) = result {
                candidates.push((t.to, t.confidence, self.ensemble_weights[2]));
            }
            result
        } else {
            None
        };

        // Ensemble: weighted vote.
        let (state, confidence) = if candidates.is_empty() {
            (CognitiveState::Unknown, 0.0)
        } else {
            weighted_vote(&candidates)
        };

        // Clinical scoring.
        let mut brain_health_index = None;
        let mut clinical_flags = Vec::new();

        if let Some(ref clinical) = self.clinical {
            let health = clinical.brain_health_index(metrics);
            brain_health_index = Some(health);

            let alz = clinical.alzheimer_risk(metrics);
            let epi = clinical.epilepsy_risk(metrics);
            let dep = clinical.depression_risk(metrics);

            if alz > 0.7 {
                clinical_flags.push(format!("Elevated Alzheimer risk: {:.2}", alz));
            }
            if epi > 0.7 {
                clinical_flags.push(format!("Elevated epilepsy risk: {:.2}", epi));
            }
            if dep > 0.7 {
                clinical_flags.push(format!("Elevated depression risk: {:.2}", dep));
            }
            if health < 0.3 {
                clinical_flags.push(format!("Low brain health index: {:.2}", health));
            }
        }

        DecoderOutput {
            state,
            confidence,
            transition,
            brain_health_index,
            clinical_flags,
            timestamp: metrics.timestamp,
        }
    }
}

impl Default for DecoderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Weighted majority vote across candidate predictions.
///
/// Returns the state with the highest weighted confidence and the
/// normalized confidence score.
fn weighted_vote(candidates: &[(CognitiveState, f64, f64)]) -> (CognitiveState, f64) {
    use std::collections::HashMap;

    let mut state_scores: HashMap<CognitiveState, f64> = HashMap::new();
    let mut total_weight = 0.0;

    for &(state, confidence, weight) in candidates {
        let score = confidence * weight;
        *state_scores.entry(state).or_insert(0.0) += score;
        total_weight += score;
    }

    let (best_state, best_score) = state_scores
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((CognitiveState::Unknown, 0.0));

    let normalized = if total_weight > 0.0 {
        (best_score / total_weight).clamp(0.0, 1.0)
    } else {
        0.0
    };

    (best_state, normalized)
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

    fn make_metrics(mincut: f64, modularity: f64) -> TopologyMetrics {
        TopologyMetrics {
            global_mincut: mincut,
            modularity,
            global_efficiency: 0.3,
            local_efficiency: 0.2,
            graph_entropy: 2.0,
            fiedler_value: 0.5,
            num_modules: 4,
            timestamp: 0.0,
        }
    }

    #[test]
    fn test_empty_pipeline() {
        let mut pipeline = DecoderPipeline::new();
        let emb = make_embedding(vec![1.0, 0.0]);
        let met = make_metrics(5.0, 0.4);
        let output = pipeline.decode(&emb, &met);
        assert_eq!(output.state, CognitiveState::Unknown);
        assert!(output.confidence >= 0.0 && output.confidence <= 1.0);
    }

    #[test]
    fn test_pipeline_with_knn() {
        let mut pipeline = DecoderPipeline::new().with_knn(3);
        pipeline.knn_mut().unwrap().train(vec![
            (make_embedding(vec![1.0, 0.0]), CognitiveState::Rest),
            (make_embedding(vec![1.1, 0.1]), CognitiveState::Rest),
            (make_embedding(vec![0.9, 0.0]), CognitiveState::Rest),
        ]);

        let output = pipeline.decode(&make_embedding(vec![1.0, 0.05]), &make_metrics(5.0, 0.4));
        assert_eq!(output.state, CognitiveState::Rest);
        assert!(output.confidence > 0.0);
    }

    #[test]
    fn test_pipeline_with_thresholds() {
        let mut pipeline = DecoderPipeline::new().with_thresholds();
        pipeline.threshold_mut().unwrap().set_threshold(
            CognitiveState::Focused,
            crate::threshold_decoder::TopologyThreshold {
                mincut_range: (7.0, 9.0),
                modularity_range: (0.5, 0.7),
                efficiency_range: (0.2, 0.4),
                entropy_range: (1.5, 2.5),
            },
        );

        let output = pipeline.decode(
            &make_embedding(vec![0.5, 0.5]),
            &make_metrics(8.0, 0.6),
        );
        assert_eq!(output.state, CognitiveState::Focused);
    }

    #[test]
    fn test_pipeline_with_clinical() {
        let baseline = make_metrics(5.0, 0.4);
        let std_met = TopologyMetrics {
            global_mincut: 1.0,
            modularity: 0.1,
            global_efficiency: 0.05,
            local_efficiency: 0.05,
            graph_entropy: 0.3,
            fiedler_value: 0.1,
            num_modules: 1,
            timestamp: 0.0,
        };
        let mut pipeline = DecoderPipeline::new()
            .with_knn(1)
            .with_clinical(baseline, std_met);
        pipeline.knn_mut().unwrap().train(vec![(
            make_embedding(vec![1.0]),
            CognitiveState::Rest,
        )]);

        let output = pipeline.decode(&make_embedding(vec![1.0]), &make_metrics(5.0, 0.4));
        assert!(output.brain_health_index.is_some());
        let health = output.brain_health_index.unwrap();
        assert!(health >= 0.0 && health <= 1.0);
    }

    #[test]
    fn test_pipeline_all_decoders() {
        let baseline = make_metrics(5.0, 0.4);
        let std_met = TopologyMetrics {
            global_mincut: 1.0,
            modularity: 0.1,
            global_efficiency: 0.05,
            local_efficiency: 0.05,
            graph_entropy: 0.3,
            fiedler_value: 0.1,
            num_modules: 1,
            timestamp: 0.0,
        };
        let mut pipeline = DecoderPipeline::new()
            .with_knn(3)
            .with_thresholds()
            .with_transitions(5)
            .with_clinical(baseline, std_met);

        pipeline.knn_mut().unwrap().train(vec![
            (make_embedding(vec![1.0, 0.0]), CognitiveState::Rest),
            (make_embedding(vec![1.1, 0.1]), CognitiveState::Rest),
        ]);

        let output = pipeline.decode(&make_embedding(vec![1.0, 0.05]), &make_metrics(5.0, 0.4));
        // Should produce some output regardless of which decoders fire.
        assert!(output.confidence >= 0.0 && output.confidence <= 1.0);
        assert!(output.brain_health_index.is_some());
    }

    #[test]
    fn test_decoder_output_serialization() {
        let output = DecoderOutput {
            state: CognitiveState::Rest,
            confidence: 0.95,
            transition: None,
            brain_health_index: Some(0.92),
            clinical_flags: vec![],
            timestamp: 1234.5,
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: DecoderOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.state, CognitiveState::Rest);
        assert!((parsed.confidence - 0.95).abs() < 1e-10);
    }
}

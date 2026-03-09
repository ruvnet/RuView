//! rUv Neural Decoder -- Cognitive state classification and BCI decoding
//! from neural topology embeddings.
//!
//! This crate provides multiple decoding strategies for classifying cognitive
//! states from brain graph embeddings and topology metrics:
//!
//! - **KNN Decoder**: K-nearest neighbor classification using stored labeled embeddings
//! - **Threshold Decoder**: Rule-based classification from topology metric ranges
//! - **Transition Decoder**: State transition detection from topology dynamics
//! - **Clinical Scorer**: Biomarker detection via deviation from healthy baselines
//! - **Pipeline**: End-to-end ensemble decoder combining all strategies

pub mod clinical;
pub mod knn_decoder;
pub mod pipeline;
pub mod threshold_decoder;
pub mod transition_decoder;

pub use clinical::ClinicalScorer;
pub use knn_decoder::KnnDecoder;
pub use pipeline::{DecoderOutput, DecoderPipeline};
pub use threshold_decoder::{ThresholdDecoder, TopologyThreshold};
pub use transition_decoder::{StateTransition, TransitionDecoder, TransitionPattern};

//! Advanced Features for Ruvector
//!
//! This module provides advanced vector database capabilities:
//! - Enhanced Product Quantization with precomputed lookup tables
//! - Filtered Search with automatic strategy selection
//! - MMR (Maximal Marginal Relevance) for diversity
//! - Hybrid Search combining vector and keyword matching
//! - Conformal Prediction for uncertainty quantification

pub mod conformal_prediction;
pub mod filtered_search;
pub mod hybrid_search;
pub mod mmr;
pub mod product_quantization;

// Re-exports
pub use conformal_prediction::{
    ConformalConfig, ConformalPredictor, NonconformityMeasure, PredictionSet,
};
pub use filtered_search::{FilterExpression, FilterStrategy, FilteredSearch};
pub use hybrid_search::{HybridConfig, HybridSearch, NormalizationStrategy, BM25};
pub use mmr::{MMRConfig, MMRSearch};
pub use product_quantization::{EnhancedPQ, LookupTable, PQConfig};

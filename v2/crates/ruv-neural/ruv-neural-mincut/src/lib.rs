//! # rUv Neural Mincut
//!
//! Dynamic minimum cut analysis for brain network topology detection.
//!
//! This crate provides algorithms for computing minimum cuts on brain connectivity
//! graphs, tracking topology changes over time, and detecting neural coherence events.
//!
//! ## Algorithms
//!
//! - **Stoer-Wagner**: Global minimum cut in O(V^3) time
//! - **Normalized cut** (Shi-Malik): Spectral bisection via the Fiedler vector
//! - **Multiway cut**: Recursive normalized cut for k-module detection
//! - **Spectral cut**: Cheeger constant, spectral bisection, Cheeger bounds
//!
//! ## Dynamic Analysis
//!
//! - **DynamicMincutTracker**: Track mincut evolution over temporal graph sequences
//! - **CoherenceDetector**: Detect network formation, dissolution, merger, and split events

pub mod benchmark;
pub mod coherence;
pub mod dynamic;
pub mod multiway;
pub mod normalized;
pub mod spectral_cut;
pub mod stoer_wagner;

// Re-export primary public API
pub use coherence::{CoherenceDetector, CoherenceEvent, CoherenceEventType};
pub use dynamic::{DynamicMincutTracker, TopologyTransition, TransitionDirection};
pub use multiway::{detect_modules, multiway_cut};
pub use normalized::normalized_cut;
pub use spectral_cut::{cheeger_bound, cheeger_constant, spectral_bisection};
pub use stoer_wagner::stoer_wagner_mincut;

// Re-export core types used in our public API
pub use ruv_neural_core::graph::{BrainGraph, BrainGraphSequence};
pub use ruv_neural_core::topology::{MincutResult, MultiPartition};
pub use ruv_neural_core::{Result, RuvNeuralError};

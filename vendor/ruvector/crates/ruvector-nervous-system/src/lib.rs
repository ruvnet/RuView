//! # RuVector Nervous System
//!
//! Biologically-inspired nervous system components for RuVector including:
//! - Dendritic coincidence detection with NMDA-like nonlinearity
//! - Hyperdimensional computing (HDC) for neural-symbolic AI
//! - Cognitive routing for multi-agent systems
//!
//! ## Dendrite Module
//!
//! Implements reduced compartment dendritic models that detect temporal coincidence
//! of synaptic inputs within 10-50ms windows. Based on Dendrify framework and
//! DenRAM RRAM circuits.
//!
//! ### Example
//!
//! ```rust
//! use ruvector_nervous_system::dendrite::{Dendrite, DendriticTree};
//!
//! // Create a dendrite with NMDA threshold of 5 synapses
//! let mut dendrite = Dendrite::new(5, 20.0);
//!
//! // Simulate coincident synaptic inputs
//! for i in 0..6 {
//!     dendrite.receive_spike(i, 100);
//! }
//!
//! // Update dendrite - should trigger plateau potential
//! let plateau_triggered = dendrite.update(100, 1.0);
//! assert!(plateau_triggered);
//! ```
//!
//! ## HDC Module
//!
//! High-performance hyperdimensional computing implementation with SIMD-optimized
//! operations for neural-symbolic AI.
//!
//! ### Example
//!
//! ```rust
//! use ruvector_nervous_system::hdc::{Hypervector, HdcMemory};
//!
//! // Create random hypervectors
//! let v1 = Hypervector::random();
//! let v2 = Hypervector::random();
//!
//! // Bind vectors with XOR
//! let bound = v1.bind(&v2);
//!
//! // Compute similarity (0.0 to 1.0)
//! let sim = v1.similarity(&v2);
//! ```
//!
//! ## EventBus Module
//!
//! Lock-free event queue system for DVS (Dynamic Vision Sensor) event streams
//! with 10,000+ events/millisecond throughput.
//!
//! ### Example
//!
//! ```rust
//! use ruvector_nervous_system::eventbus::{DVSEvent, EventRingBuffer, ShardedEventBus};
//!
//! // Create event
//! let event = DVSEvent::new(1000, 42, 123, true);
//!
//! // Lock-free ring buffer
//! let buffer = EventRingBuffer::new(1024);
//! buffer.push(event).unwrap();
//!
//! // Sharded bus for parallel processing
//! let bus = ShardedEventBus::new_spatial(4, 256);
//! bus.push(event).unwrap();
//! ```

pub mod compete;
pub mod dendrite;
pub mod eventbus;
pub mod hdc;
pub mod hopfield;
pub mod integration;
pub mod plasticity;
pub mod routing;
pub mod separate;

pub use compete::{KWTALayer, LateralInhibition, WTALayer};
pub use dendrite::{Compartment, Dendrite, DendriticTree, PlateauPotential};
pub use eventbus::{
    BackpressureController, BackpressureState, DVSEvent, Event, EventRingBuffer, EventSurface,
    ShardedEventBus,
};
pub use hdc::{HdcError, HdcMemory, Hypervector};
pub use hopfield::ModernHopfield;
pub use plasticity::eprop::{EpropLIF, EpropNetwork, EpropSynapse, LearningSignal};
pub use routing::{
    BudgetGuardrail, CircadianController, CircadianPhase, CircadianScheduler, CoherenceGatedSystem,
    GlobalWorkspace, HysteresisTracker, NervousSystemMetrics, NervousSystemScorecard,
    OscillatoryRouter, PhaseModulation, PredictiveLayer, Representation, ScorecardTargets,
};
pub use separate::{DentateGyrus, SparseBitVector, SparseProjection};

#[derive(Debug, thiserror::Error)]
pub enum NervousSystemError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Compartment index out of bounds: {0}")]
    CompartmentOutOfBounds(usize),

    #[error("Synapse index out of bounds: {0}")]
    SynapseOutOfBounds(usize),

    #[error("Invalid weight: {0}")]
    InvalidWeight(f32),

    #[error("Invalid time constant: {0}")]
    InvalidTimeConstant(f32),

    #[error("Invalid gradients: {0}")]
    InvalidGradients(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("HDC error: {0}")]
    HdcError(#[from] HdcError),

    #[error("Invalid dimension: {0}")]
    InvalidDimension(String),

    #[error("Invalid sparsity: {0}")]
    InvalidSparsity(String),
}

pub type Result<T> = std::result::Result<T, NervousSystemError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_hdc_workflow() {
        let v1 = Hypervector::random();
        let v2 = Hypervector::random();

        // Similarity of random vectors should be ~0.0 (50% bit overlap)
        // Formula: 1 - 2*hamming/dim = 1 - 2*0.5 = 0
        let sim = v1.similarity(&v2);
        assert!(sim > -0.2 && sim < 0.2, "random similarity: {}", sim);

        // Binding produces ~0 similarity with original
        let bound = v1.bind(&v2);
        assert!(
            bound.similarity(&v1) > -0.2,
            "bound similarity: {}",
            bound.similarity(&v1)
        );

        // Memory
        let mut memory = HdcMemory::new();
        memory.store("test", v1.clone());
        let results = memory.retrieve(&v1, 0.9);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "test");
    }

    #[test]
    fn test_dendrite_workflow() {
        let mut dendrite = Dendrite::new(5, 20.0);

        // Insufficient spikes - no plateau
        for i in 0..3 {
            dendrite.receive_spike(i, 100);
        }
        let triggered = dendrite.update(100, 1.0);
        assert!(!triggered);

        // Sufficient spikes - trigger plateau
        for i in 3..8 {
            dendrite.receive_spike(i, 100);
        }
        let triggered = dendrite.update(100, 1.0);
        assert!(triggered);
        assert!(dendrite.has_plateau());
    }
}

//! Coherence Computation Engine
//!
//! This module implements the core coherence computation using sheaf Laplacian mathematics.
//! The key formula is: E(S) = sum(w_e * |r_e|^2) where r_e = rho_u(x_u) - rho_v(x_v)
//!
//! # Architecture
//!
//! ```text
//! +-------------------+
//! |  CoherenceEngine  |  Aggregate with residual cache
//! +-------------------+
//!          |
//!   +------+------+
//!   |             |
//!   v             v
//! +-------+  +-----------+
//! | Energy |  | Spectral  |  Value objects and analyzers
//! +-------+  +-----------+
//!          |
//!          v
//! +-------------------+
//! |   Incremental     |  Efficient delta computation
//! +-------------------+
//! ```
//!
//! # Features
//!
//! - **Parallel Computation**: Uses rayon for parallel residual calculation
//! - **Fingerprint-Based Staleness**: Detects when recomputation is needed
//! - **Hotspot Identification**: Finds highest energy edges
//! - **SIMD Optimization**: Fast residual norm computation
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::coherence::{CoherenceEngine, CoherenceConfig};
//!
//! let config = CoherenceConfig::default();
//! let mut engine = CoherenceEngine::new(config);
//!
//! // Add nodes and edges
//! engine.add_node("fact1", vec![1.0, 0.5, 0.3]);
//! engine.add_node("fact2", vec![0.9, 0.6, 0.2]);
//! engine.add_edge("fact1", "fact2", 1.0, None);
//!
//! // Compute coherence
//! let energy = engine.compute_energy();
//! println!("Total coherence energy: {}", energy.total_energy);
//!
//! // Update incrementally
//! engine.update_node("fact1", vec![1.0, 0.5, 0.4]);
//! let updated = engine.compute_incremental();
//! ```

mod energy;
mod engine;
mod history;
mod incremental;
mod spectral;

pub use energy::{
    compute_norm_sq, compute_residual, CoherenceEnergy, EdgeEnergy, EnergySnapshot,
    EnergyStatistics, HotspotInfo, ScopeEnergy, ScopeId,
};
pub use engine::{
    CoherenceConfig, CoherenceEngine, CoherenceError, NodeState, RestrictionMap, Result, SheafEdge,
    SheafNode,
};
pub use history::{EnergyHistory, EnergyHistoryConfig, EnergyTrend, TrendDirection};
pub use incremental::{
    DeltaResult, IncrementalCache, IncrementalConfig, IncrementalEngine, UpdateEvent,
};
pub use spectral::{
    compute_eigenvalues, DriftEvent, DriftSeverity, SpectralAnalyzer, SpectralConfig, SpectralStats,
};

// Alias for compatibility
pub use incremental::IncrementalCache as ResidualCache;

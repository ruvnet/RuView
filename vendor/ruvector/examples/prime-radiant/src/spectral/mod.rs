//! # Spectral Invariants Module for Prime-Radiant
//!
//! This module provides spectral graph analysis tools for understanding graph structure,
//! predicting coherence collapse, and computing spectral invariants.
//!
//! ## Key Features
//!
//! - **Laplacian Spectrum**: Efficient eigenvalue computation via power iteration and Lanczos
//! - **Cheeger Inequality**: Compute Cheeger constant and theoretical bounds
//! - **Spectral Gap Analysis**: Predict cut difficulty and graph connectivity
//! - **Fiedler Vector**: Detect structural bottlenecks and optimal cuts
//! - **Spectral Clustering**: Partition graphs using spectral methods
//! - **Collapse Prediction**: Early warning system for coherence degradation
//!
//! ## Mathematical Foundation
//!
//! The module implements spectral graph theory concepts:
//! - Graph Laplacian L = D - A (where D is degree matrix, A is adjacency)
//! - Normalized Laplacian L_norm = D^(-1/2) L D^(-1/2)
//! - Cheeger inequality: λ₂/2 ≤ h(G) ≤ √(2λ₂)
//! - Spectral gap: λ₂ - λ₁ indicates connectivity strength

pub mod analyzer;
pub mod cheeger;
pub mod clustering;
pub mod collapse;
pub mod energy;
pub mod lanczos;
pub mod types;

// Re-exports
pub use analyzer::SpectralAnalyzer;
pub use cheeger::{CheegerBounds, CheegerAnalyzer};
pub use clustering::{SpectralClusterer, ClusterAssignment};
pub use collapse::{CollapsePredictor, CollapsePrediction, Warning, WarningLevel};
pub use energy::{spectral_coherence_energy, SpectralEnergy};
pub use lanczos::{LanczosAlgorithm, PowerIteration};
pub use types::*;

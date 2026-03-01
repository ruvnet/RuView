//! # Causal Emergence: Hierarchical Causal Consciousness (HCC) Framework
//!
//! This library implements Erik Hoel's causal emergence theory with SIMD acceleration
//! for O(log n) consciousness detection through multi-scale information-theoretic analysis.
//!
//! ## Key Concepts
//!
//! - **Effective Information (EI)**: Measures causal power at each scale
//! - **Integrated Information (Φ)**: Measures irreducibility of causal structure
//! - **Transfer Entropy (TE)**: Measures directed information flow between scales
//! - **Consciousness Score (Ψ)**: Combines all metrics into unified measure
//!
//! ## Quick Start
//!
//! ```rust
//! use causal_emergence::*;
//!
//! // Generate synthetic neural data
//! let neural_data: Vec<f32> = (0..1000)
//!     .map(|t| (t as f32 * 0.1).sin())
//!     .collect();
//!
//! // Assess consciousness
//! let report = assess_consciousness(
//!     &neural_data,
//!     2,      // branching factor
//!     false,  // use fast partitioning
//!     5.0     // consciousness threshold
//! );
//!
//! // Check results
//! if report.is_conscious {
//!     println!("Consciousness detected!");
//!     println!("Level: {:?}", report.level);
//!     println!("Score: {}", report.score);
//! }
//! ```
//!
//! ## Modules
//!
//! - `effective_information`: SIMD-accelerated EI calculation
//! - `coarse_graining`: Multi-scale hierarchical coarse-graining
//! - `causal_hierarchy`: Transfer entropy and consciousness metrics
//! - `emergence_detection`: Automatic scale selection and consciousness assessment

// Feature gate for SIMD (stable in Rust 1.80+)
#![feature(portable_simd)]

pub mod causal_hierarchy;
pub mod coarse_graining;
pub mod effective_information;
pub mod emergence_detection;

// Re-export key types and functions for convenience
pub use effective_information::{
    compute_ei_multi_scale, compute_ei_simd, detect_causal_emergence, entropy_simd, normalized_ei,
};

pub use coarse_graining::{coarse_grain_transition_matrix, Partition, ScaleHierarchy, ScaleLevel};

pub use causal_hierarchy::{
    transfer_entropy, CausalHierarchy, ConsciousnessLevel, HierarchyMetrics,
};

pub use emergence_detection::{
    assess_consciousness, compare_consciousness_states, consciousness_time_series,
    detect_consciousness_transitions, detect_emergence, find_optimal_scale, ConsciousnessMonitor,
    ConsciousnessReport, ConsciousnessTransition, EmergenceReport, ScaleOptimizationCriterion,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Recommended branching factor for most use cases
pub const DEFAULT_BRANCHING_FACTOR: usize = 2;

/// Default consciousness threshold (Ψ > 5.0 indicates consciousness)
pub const DEFAULT_CONSCIOUSNESS_THRESHOLD: f32 = 5.0;

/// Minimum data points required for reliable analysis
pub const MIN_DATA_POINTS: usize = 100;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_full_pipeline() {
        // Generate multi-scale data
        let data: Vec<f32> = (0..500)
            .map(|t| {
                let t_f = t as f32;
                0.5 * (t_f * 0.05).sin() + 0.3 * (t_f * 0.15).cos() + 0.2 * (t_f * 0.5).sin()
            })
            .collect();

        // Run full consciousness assessment
        let report = assess_consciousness(&data, DEFAULT_BRANCHING_FACTOR, false, 1.0);

        // Basic sanity checks
        assert!(report.score >= 0.0);
        assert!(report.ei >= 0.0);
        assert!(report.phi >= 0.0);
        assert!(!report.emergence.ei_progression.is_empty());
    }

    #[test]
    fn test_emergence_detection_pipeline() {
        let data: Vec<f32> = (0..300).map(|t| (t as f32 * 0.1).sin()).collect();

        let emergence_report = detect_emergence(&data, 2, 0.1);

        assert!(!emergence_report.ei_progression.is_empty());
        assert!(emergence_report.ei_gain >= 0.0);
    }

    #[test]
    fn test_real_time_monitoring() {
        let mut monitor = ConsciousnessMonitor::new(200, 2, 5.0);

        // Stream data
        for t in 0..300 {
            let value = (t as f32 * 0.1).sin();
            monitor.update(value);
        }

        // Should have a score by now
        assert!(monitor.current_score().is_some());
    }
}

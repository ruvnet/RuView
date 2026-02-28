//! # ruvector-crv
//!
//! CRV (Coordinate Remote Viewing) protocol integration for ruvector.
//!
//! Maps the 6-stage CRV signal line methodology to ruvector's subsystems:
//!
//! | CRV Stage | Data Type | ruvector Component |
//! |-----------|-----------|-------------------|
//! | Stage I (Ideograms) | Gestalt primitives | Poincaré ball hyperbolic embeddings |
//! | Stage II (Sensory) | Textures, colors, temps | Multi-head attention vectors |
//! | Stage III (Dimensional) | Spatial sketches | GNN graph topology |
//! | Stage IV (Emotional) | AOL, intangibles | SNN temporal encoding |
//! | Stage V (Interrogation) | Signal line probing | Differentiable search |
//! | Stage VI (3D Model) | Composite model | MinCut partitioning |
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ruvector_crv::{CrvConfig, CrvSessionManager, GestaltType, StageIData};
//!
//! // Create session manager with default config (384 dimensions)
//! let config = CrvConfig::default();
//! let mut manager = CrvSessionManager::new(config);
//!
//! // Create a session for a target coordinate
//! manager.create_session("session-001".to_string(), "1234-5678".to_string()).unwrap();
//!
//! // Add Stage I ideogram data
//! let stage_i = StageIData {
//!     stroke: vec![(0.0, 0.0), (1.0, 0.5), (2.0, 1.0), (3.0, 0.5)],
//!     spontaneous_descriptor: "angular rising".to_string(),
//!     classification: GestaltType::Manmade,
//!     confidence: 0.85,
//! };
//!
//! let embedding = manager.add_stage_i("session-001", &stage_i).unwrap();
//! assert_eq!(embedding.len(), 384);
//! ```
//!
//! ## Architecture
//!
//! The Poincaré ball embedding for Stage I gestalts encodes the hierarchical
//! gestalt taxonomy (root → manmade/natural/movement/energy/water/land) with
//! exponentially less distortion than Euclidean space.
//!
//! For AOL (Analytical Overlay) separation, the spiking neural network temporal
//! encoding models signal-vs-noise discrimination: high-frequency spike bursts
//! correlate with AOL contamination, while sustained low-frequency patterns
//! indicate clean signal line data.
//!
//! MinCut partitioning in Stage VI identifies natural cluster boundaries in the
//! accumulated session graph, separating distinct target aspects.
//!
//! ## Cross-Session Convergence
//!
//! Multiple sessions targeting the same coordinate can be analyzed for
//! convergence — agreement between independent viewers strengthens the
//! signal validity:
//!
//! ```rust,no_run
//! # use ruvector_crv::{CrvConfig, CrvSessionManager};
//! # let mut manager = CrvSessionManager::new(CrvConfig::default());
//! // After adding data to multiple sessions for "1234-5678"...
//! let convergence = manager.find_convergence("1234-5678", 0.75).unwrap();
//! // convergence.scores contains similarity values for converging entries
//! ```

pub mod error;
pub mod session;
pub mod stage_i;
pub mod stage_ii;
pub mod stage_iii;
pub mod stage_iv;
pub mod stage_v;
pub mod stage_vi;
pub mod types;

// Re-export main types
pub use error::{CrvError, CrvResult};
pub use session::CrvSessionManager;
pub use stage_i::StageIEncoder;
pub use stage_ii::StageIIEncoder;
pub use stage_iii::StageIIIEncoder;
pub use stage_iv::StageIVEncoder;
pub use stage_v::StageVEngine;
pub use stage_vi::StageVIModeler;
pub use types::{
    AOLDetection, ConvergenceResult, CrossReference, CrvConfig, CrvSessionEntry, GeometricKind,
    GestaltType, SensoryModality, SignalLineProbe, SketchElement, SpatialRelationType,
    SpatialRelationship, StageIData, StageIIData, StageIIIData, StageIVData, StageVData,
    StageVIData, TargetPartition,
};

/// Library version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_end_to_end_session() {
        let config = CrvConfig {
            dimensions: 32,
            ..CrvConfig::default()
        };
        let mut manager = CrvSessionManager::new(config);

        // Create two sessions for the same coordinate
        manager
            .create_session("viewer-a".to_string(), "target-001".to_string())
            .unwrap();
        manager
            .create_session("viewer-b".to_string(), "target-001".to_string())
            .unwrap();

        // Viewer A: Stage I
        let s1_a = StageIData {
            stroke: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5), (3.0, 0.0)],
            spontaneous_descriptor: "tall angular".to_string(),
            classification: GestaltType::Manmade,
            confidence: 0.85,
        };
        manager.add_stage_i("viewer-a", &s1_a).unwrap();

        // Viewer B: Stage I (similar gestalt)
        let s1_b = StageIData {
            stroke: vec![(0.0, 0.0), (0.5, 1.2), (1.5, 0.8), (2.5, 0.0)],
            spontaneous_descriptor: "structured upward".to_string(),
            classification: GestaltType::Manmade,
            confidence: 0.78,
        };
        manager.add_stage_i("viewer-b", &s1_b).unwrap();

        // Viewer A: Stage II
        let s2_a = StageIIData {
            impressions: vec![
                (SensoryModality::Texture, "rough stone".to_string()),
                (SensoryModality::Temperature, "cool".to_string()),
                (SensoryModality::Color, "gray".to_string()),
            ],
            feature_vector: None,
        };
        manager.add_stage_ii("viewer-a", &s2_a).unwrap();

        // Viewer B: Stage II (overlapping sensory)
        let s2_b = StageIIData {
            impressions: vec![
                (SensoryModality::Texture, "grainy rough".to_string()),
                (SensoryModality::Color, "dark gray".to_string()),
                (SensoryModality::Luminosity, "dim".to_string()),
            ],
            feature_vector: None,
        };
        manager.add_stage_ii("viewer-b", &s2_b).unwrap();

        // Verify entries
        assert_eq!(manager.session_entry_count("viewer-a"), 2);
        assert_eq!(manager.session_entry_count("viewer-b"), 2);

        // Both sessions should have embeddings
        let entries_a = manager.get_session_embeddings("viewer-a").unwrap();
        let entries_b = manager.get_session_embeddings("viewer-b").unwrap();

        assert_eq!(entries_a.len(), 2);
        assert_eq!(entries_b.len(), 2);

        // All embeddings should be 32-dimensional
        for entry in entries_a.iter().chain(entries_b.iter()) {
            assert_eq!(entry.embedding.len(), 32);
        }
    }
}

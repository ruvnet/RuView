//! Signal Intelligence pipeline (Phase 2, ADR-022).
//!
//! Composes `RuVector` primitives into a multi-stage sensing pipeline
//! that transforms multi-BSSID RSSI frames into presence, motion,
//! and coarse vital sign estimates.
//!
//! ## Stages
//!
//! 1. [`predictive_gate`] -- residual gating via `PredictiveLayer`
//! 2. [`attention_weighter`] -- BSSID attention weighting
//! 3. [`correlator`] -- cross-BSSID Pearson correlation & clustering
//! 4. [`motion_estimator`] -- multi-AP motion estimation
//! 5. [`breathing_extractor`] -- coarse breathing rate extraction
//! 6. [`quality_gate`] -- ruQu three-filter quality gate
//! 7. [`fingerprint_matcher`] -- `ModernHopfield` posture fingerprinting
//! 8. [`orchestrator`] -- full pipeline orchestrator

#[cfg(feature = "pipeline")]
pub mod predictive_gate;
#[cfg(feature = "pipeline")]
pub mod attention_weighter;
#[cfg(feature = "pipeline")]
pub mod correlator;
#[cfg(feature = "pipeline")]
pub mod motion_estimator;
#[cfg(feature = "pipeline")]
pub mod breathing_extractor;
#[cfg(feature = "pipeline")]
pub mod quality_gate;
#[cfg(feature = "pipeline")]
pub mod fingerprint_matcher;
#[cfg(feature = "pipeline")]
pub mod orchestrator;

#[cfg(feature = "pipeline")]
pub use orchestrator::WindowsWifiPipeline;

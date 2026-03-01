//! SONA Tuning Integration - Self-Optimizing Threshold Learning
//!
//! This module provides integration with the `sona` crate for adaptive threshold
//! learning in the coherence engine. SONA (Self-Optimizing Neural Architecture)
//! enables the coherence gate thresholds to adapt based on operational experience.
//!
//! # Architecture
//!
//! The SONA integration provides three learning loops:
//!
//! 1. **Instant Loop** (Micro-LoRA): Ultra-low latency (<0.05ms) adaptation
//! 2. **Background Loop** (Base-LoRA): Deeper learning in background threads
//! 3. **Coordination Loop**: Synchronizes instant and background learning
//!
//! # Key Types
//!
//! - [`SonaThresholdTuner`]: Main adapter for threshold learning
//! - [`ThresholdConfig`]: Threshold configuration for compute lanes
//! - [`ThresholdAdjustment`]: Recommended threshold changes
//! - [`TunerConfig`]: Configuration for the SONA integration
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::sona_tuning::{SonaThresholdTuner, TunerConfig};
//!
//! // Create threshold tuner
//! let mut tuner = SonaThresholdTuner::new(TunerConfig::default());
//!
//! // Begin tracking a new regime
//! let builder = tuner.begin_regime(&energy_trace);
//!
//! // After observing outcome, learn from it
//! tuner.learn_outcome(builder, success_score);
//!
//! // Query for similar past configurations
//! if let Some(config) = tuner.find_similar_regime(&current_energy) {
//!     // Apply recommended thresholds
//! }
//! ```

mod adjustment;
mod config;
mod error;
mod tuner;

pub use adjustment::{AdjustmentReason, ThresholdAdjustment};
pub use config::{LearningLoopConfig, ThresholdConfig, TunerConfig};
pub use error::{SonaTuningError, SonaTuningResult};
pub use tuner::{RegimeTracker, SonaThresholdTuner, TunerState};

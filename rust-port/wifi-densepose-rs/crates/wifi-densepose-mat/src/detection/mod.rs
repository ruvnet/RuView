//! Detection module for vital signs detection from CSI data.
//!
//! This module provides detectors for:
//! - Breathing patterns
//! - Heartbeat signatures
//! - Movement classification
//! - Ensemble classification combining all signals

mod breathing;
mod heartbeat;
mod movement;
mod pipeline;

pub use breathing::{BreathingDetector, BreathingDetectorConfig};
pub use heartbeat::{HeartbeatDetector, HeartbeatDetectorConfig};
pub use movement::{MovementClassifier, MovementClassifierConfig};
pub use pipeline::{DetectionPipeline, DetectionConfig, VitalSignsDetector, CsiDataBuffer};

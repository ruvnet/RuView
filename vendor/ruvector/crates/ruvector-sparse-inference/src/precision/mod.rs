//! Precision Lanes Module - Layered Quantization for Sparse Inference
//!
//! This module implements a 3/5/7-bit layered quantization system that turns
//! activation locality into a complete control theory for inference.
//!
//! # Intelligence Roles by Precision Lane
//!
//! - **3-bit Lane**: Reflex signals, gating, anomaly boundaries, mincut triggers, health metrics
//! - **5-bit Lane**: Streaming embeddings, semantic motion, drift detection, lightweight perception
//! - **7-bit Lane**: Reasoning, synthesis, memory writes, micro-LoRA adaptation, summaries
//! - **Float Lane**: Training, offline calibration, rare aggregation boundaries
//!
//! # Graduation Rules
//!
//! Signals move UP lanes when:
//! - Novelty exceeds threshold
//! - Drift persists for N steps
//! - Confidence and stability metrics pass
//! - Cost budget allows escalation
//!
//! Signals move DOWN lanes when:
//! - Stability returns
//! - Velocity stalls
//! - Active set shrinks
//! - Uncertainty is high but no action needed
//!
//! # Key Insight
//!
//! The active neuron set decides WHAT to compute.
//! The lane decides HOW PRECISELY to compute it.
//! The graduation rules decide WHEN computation is allowed to become expensive.

pub mod lanes;
pub mod policy;
pub mod quantizers;
pub mod telemetry;

pub use lanes::{LaneConfig, PrecisionLane};
pub use policy::{GraduationDecision, GraduationMetrics, GraduationPolicy};
pub use quantizers::{QuantizedBlock, Quantizer3Bit, Quantizer5Bit, Quantizer7Bit};
pub use telemetry::{LaneStats, LaneTelemetry};

//! WiFi-DensePose Sensing Server library.
//!
//! This crate provides:
//! - Vital sign detection from WiFi CSI amplitude data
//! - RVF (RuVector Format) binary container for model weights
//! - Opt-in bearer-token auth for the `/api/v1/*` HTTP surface (`bearer_auth`)
//! - Real-time CSI introspection / low-latency tap (`introspection`, ADR-099)

pub mod bearer_auth;
pub mod introspection;
pub mod vital_signs;
pub mod rvf_container;
pub mod rvf_pipeline;
pub mod graph_transformer;
#[allow(dead_code)]
pub mod trainer;
pub mod dataset;
pub mod sona;
pub mod sparse_inference;
#[allow(dead_code)]
pub mod embedding;

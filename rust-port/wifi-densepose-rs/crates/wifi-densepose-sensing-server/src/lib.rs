//! WiFi-DensePose Sensing Server library.
//!
//! This crate provides:
//! - Vital sign detection from WiFi CSI amplitude data
//! - RVF (RuVector Format) binary container for model weights

pub mod vital_signs;
pub mod rvf_container;
pub mod rvf_pipeline;
pub mod graph_transformer;
pub mod trainer;
pub mod dataset;
pub mod sona;
pub mod sparse_inference;
pub mod embedding;

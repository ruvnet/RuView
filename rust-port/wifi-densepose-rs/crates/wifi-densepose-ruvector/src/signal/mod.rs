//! CSI signal processing using RuVector v2.0.4.
//!
//! This module provides four integration points that augment the WiFi-DensePose
//! signal pipeline with ruvector algorithms:
//!
//! - [`subcarrier`]: Graph min-cut partitioning of subcarriers into sensitive /
//!   insensitive groups.
//! - [`spectrogram`]: Attention-guided min-cut gating that suppresses noise
//!   frames and amplifies body-motion periods.
//! - [`bvp`]: Scaled dot-product attention over subcarrier STFT rows for
//!   weighted BVP aggregation.
//! - [`fresnel`]: Sparse regularized least-squares Fresnel geometry estimation
//!   from multi-subcarrier observations.

pub mod bvp;
pub mod fresnel;
pub mod spectrogram;
pub mod subcarrier;

pub use bvp::attention_weighted_bvp;
pub use fresnel::solve_fresnel_geometry;
pub use spectrogram::gate_spectrogram;
pub use subcarrier::mincut_subcarrier_partition;

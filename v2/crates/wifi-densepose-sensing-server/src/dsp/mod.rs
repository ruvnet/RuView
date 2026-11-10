//! Digital Signal Processing (DSP) and feature extraction pipeline for WiFi CSI.

pub mod breathing;
pub mod dominant_freq;
pub mod features;
pub mod signal_field;
pub mod subcarrier;

pub use breathing::estimate_breathing_rate_hz;
pub use dominant_freq::estimate_temporal_dominant_frequency_hz;
pub use features::{extract_features_from_frame, raw_classify};
pub use signal_field::generate_signal_field;
pub use subcarrier::{compute_subcarrier_importance_weights, compute_subcarrier_variances};


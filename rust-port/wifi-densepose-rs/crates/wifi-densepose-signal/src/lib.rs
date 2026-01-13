//! WiFi-DensePose Signal Processing Library
//!
//! This crate provides signal processing capabilities for WiFi-based human pose estimation,
//! including CSI (Channel State Information) processing, phase sanitization, feature extraction,
//! and motion detection.
//!
//! # Features
//!
//! - **CSI Processing**: Preprocessing, noise removal, windowing, and normalization
//! - **Phase Sanitization**: Phase unwrapping, outlier removal, and smoothing
//! - **Feature Extraction**: Amplitude, phase, correlation, Doppler, and PSD features
//! - **Motion Detection**: Human presence detection with confidence scoring
//!
//! # Example
//!
//! ```rust,no_run
//! use wifi_densepose_signal::{
//!     CsiProcessor, CsiProcessorConfig,
//!     PhaseSanitizer, PhaseSanitizerConfig,
//!     MotionDetector,
//! };
//!
//! // Configure CSI processor
//! let config = CsiProcessorConfig::builder()
//!     .sampling_rate(1000.0)
//!     .window_size(256)
//!     .overlap(0.5)
//!     .noise_threshold(-30.0)
//!     .build();
//!
//! let processor = CsiProcessor::new(config);
//! ```

pub mod csi_processor;
pub mod features;
pub mod motion;
pub mod phase_sanitizer;

// Re-export main types for convenience
pub use csi_processor::{
    CsiData, CsiDataBuilder, CsiPreprocessor, CsiProcessor, CsiProcessorConfig,
    CsiProcessorConfigBuilder, CsiProcessorError,
};
pub use features::{
    AmplitudeFeatures, CsiFeatures, CorrelationFeatures, DopplerFeatures, FeatureExtractor,
    FeatureExtractorConfig, PhaseFeatures, PowerSpectralDensity,
};
pub use motion::{
    HumanDetectionResult, MotionAnalysis, MotionDetector, MotionDetectorConfig, MotionScore,
};
pub use phase_sanitizer::{
    PhaseSanitizationError, PhaseSanitizer, PhaseSanitizerConfig, UnwrappingMethod,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Common result type for signal processing operations
pub type Result<T> = std::result::Result<T, SignalError>;

/// Unified error type for signal processing operations
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    /// CSI processing error
    #[error("CSI processing error: {0}")]
    CsiProcessing(#[from] CsiProcessorError),

    /// Phase sanitization error
    #[error("Phase sanitization error: {0}")]
    PhaseSanitization(#[from] PhaseSanitizationError),

    /// Feature extraction error
    #[error("Feature extraction error: {0}")]
    FeatureExtraction(String),

    /// Motion detection error
    #[error("Motion detection error: {0}")]
    MotionDetection(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Data validation error
    #[error("Data validation error: {0}")]
    DataValidation(String),
}

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::csi_processor::{CsiData, CsiProcessor, CsiProcessorConfig};
    pub use crate::features::{CsiFeatures, FeatureExtractor};
    pub use crate::motion::{HumanDetectionResult, MotionDetector};
    pub use crate::phase_sanitizer::{PhaseSanitizer, PhaseSanitizerConfig};
    pub use crate::{Result, SignalError};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}

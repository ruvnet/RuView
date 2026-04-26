//! ESP32 CSI-grade vital sign extraction (ADR-021).
//!
//! Extracts heart rate and respiratory rate from WiFi Channel
//! State Information using multi-subcarrier amplitude and phase
//! analysis.
//!
//! # Architecture
//!
//! The pipeline processes CSI frames through four stages:
//!
//! 1. **Preprocessing** ([`CsiVitalPreprocessor`]): EMA-based static
//!    component suppression, producing per-subcarrier residuals.
//! 2. **Breathing extraction** ([`BreathingExtractor`]): Bandpass
//!    filtering (0.1-0.5 Hz) with zero-crossing analysis for
//!    respiratory rate.
//! 3. **Heart rate extraction** ([`HeartRateExtractor`]): Bandpass
//!    filtering (0.8-2.0 Hz) with autocorrelation peak detection
//!    and inter-subcarrier phase coherence weighting.
//! 4. **Anomaly detection** ([`VitalAnomalyDetector`]): Z-score
//!    analysis with Welford running statistics for clinical alerts
//!    (apnea, tachycardia, bradycardia).
//!
//! Results are stored in a [`VitalSignStore`] with configurable
//! retention for historical analysis.
//!
//! # Example
//!
//! ```
//! use wifi_densepose_vitals::{
//!     CsiVitalPreprocessor, BreathingExtractor, HeartRateExtractor,
//!     VitalAnomalyDetector, VitalSignStore, CsiFrame,
//!     VitalReading, VitalEstimate, VitalStatus,
//! };
//!
//! let mut preprocessor = CsiVitalPreprocessor::new(56, 0.05);
//! let mut breathing = BreathingExtractor::new(56, 100.0, 30.0);
//! let mut heartrate = HeartRateExtractor::new(56, 100.0, 15.0);
//! let mut anomaly = VitalAnomalyDetector::default_config();
//! let mut store = VitalSignStore::new(3600);
//!
//! // Process a CSI frame
//! let frame = CsiFrame {
//!     amplitudes: vec![1.0; 56],
//!     phases: vec![0.0; 56],
//!     n_subcarriers: 56,
//!     sample_index: 0,
//!     sample_rate_hz: 100.0,
//! };
//!
//! if let Some(residuals) = preprocessor.process(&frame) {
//!     let weights = vec![1.0 / 56.0; 56];
//!     let rr = breathing.extract(&residuals, &weights);
//!     let hr = heartrate.extract(&residuals, &frame.phases);
//!
//!     let reading = VitalReading {
//!         respiratory_rate: rr.unwrap_or_else(VitalEstimate::unavailable),
//!         heart_rate: hr.unwrap_or_else(VitalEstimate::unavailable),
//!         subcarrier_count: frame.n_subcarriers,
//!         signal_quality: 0.9,
//!         timestamp_secs: 0.0,
//!     };
//!
//!     let alerts = anomaly.check(&reading);
//!     store.push(reading);
//! }
//! ```

pub mod anomaly;
pub mod breathing;
pub mod heartrate;
pub mod preprocessor;
pub mod store;
pub mod types;

pub use anomaly::{AnomalyAlert, VitalAnomalyDetector};
pub use breathing::BreathingExtractor;
pub use heartrate::HeartRateExtractor;
pub use preprocessor::CsiVitalPreprocessor;
pub use store::{VitalSignStore, VitalStats};
pub use types::{CsiFrame, VitalEstimate, VitalReading, VitalStatus};

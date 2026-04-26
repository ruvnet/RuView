//! rUv Neural Signal — Digital signal processing for neural magnetic field data.
//!
//! This crate provides filtering, spectral analysis, artifact detection/rejection,
//! cross-channel connectivity metrics, and full preprocessing pipelines for
//! multi-channel neural time series data (MEG, OPM, EEG).
//!
//! # Modules
//!
//! - [`filter`] — Butterworth IIR bandpass, notch, highpass, and lowpass filters (SOS form)
//! - [`spectral`] — PSD (Welch), STFT, band power, spectral entropy, peak frequency
//! - [`hilbert`] — FFT-based Hilbert transform for instantaneous phase and amplitude
//! - [`artifact`] — Eye blink, muscle artifact, and cardiac artifact detection/rejection
//! - [`connectivity`] — PLV, coherence, imaginary coherence, amplitude envelope correlation
//! - [`preprocessing`] — Configurable multi-stage preprocessing pipeline

pub mod artifact;
pub mod connectivity;
pub mod filter;
pub mod hilbert;
pub mod preprocessing;
pub mod spectral;

pub use artifact::{detect_cardiac, detect_eye_blinks, detect_muscle_artifact, reject_artifacts};
pub use connectivity::{
    amplitude_envelope_correlation, coherence, compute_all_pairs, imaginary_coherence,
    phase_locking_value, ConnectivityMetric,
};
pub use filter::{BandpassFilter, HighpassFilter, LowpassFilter, NotchFilter, SignalProcessor};
pub use hilbert::{hilbert_transform, instantaneous_amplitude, instantaneous_phase};
pub use preprocessing::PreprocessingPipeline;
pub use spectral::{band_power, compute_psd, compute_stft, peak_frequency, spectral_entropy};

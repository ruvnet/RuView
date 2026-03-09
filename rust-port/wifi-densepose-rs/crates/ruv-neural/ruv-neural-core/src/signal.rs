//! Time series and signal types for neural data.

use serde::{Deserialize, Serialize};

use crate::error::{Result, RuvNeuralError};

/// Multi-channel time series data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiChannelTimeSeries {
    /// Raw data: `data[channel][sample]`.
    pub data: Vec<Vec<f64>>,
    /// Sampling rate in Hz.
    pub sample_rate_hz: f64,
    /// Number of channels.
    pub num_channels: usize,
    /// Number of samples per channel.
    pub num_samples: usize,
    /// Unix timestamp of the first sample.
    pub timestamp_start: f64,
}

impl MultiChannelTimeSeries {
    /// Create a new time series, validating dimensions.
    pub fn new(data: Vec<Vec<f64>>, sample_rate_hz: f64, timestamp_start: f64) -> Result<Self> {
        if !sample_rate_hz.is_finite() || sample_rate_hz <= 0.0 {
            return Err(RuvNeuralError::Signal(
                "sample_rate_hz must be finite and positive".into(),
            ));
        }
        let num_channels = data.len();
        if num_channels == 0 {
            return Err(RuvNeuralError::Signal(
                "Time series must have at least one channel".into(),
            ));
        }
        let num_samples = data[0].len();
        for (i, ch) in data.iter().enumerate() {
            if ch.len() != num_samples {
                return Err(RuvNeuralError::DimensionMismatch {
                    expected: num_samples,
                    got: ch.len(),
                });
            }
            let _ = i; // suppress unused warning
        }
        Ok(Self {
            data,
            sample_rate_hz,
            num_channels,
            num_samples,
            timestamp_start,
        })
    }

    /// Duration in seconds.
    pub fn duration_s(&self) -> f64 {
        self.num_samples as f64 / self.sample_rate_hz
    }

    /// Get a single channel's data.
    pub fn channel(&self, index: usize) -> Result<&[f64]> {
        if index >= self.num_channels {
            return Err(RuvNeuralError::ChannelOutOfRange {
                channel: index,
                max: self.num_channels.saturating_sub(1),
            });
        }
        Ok(&self.data[index])
    }
}

/// Frequency band definition for neural oscillations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FrequencyBand {
    /// Delta: 1-4 Hz (deep sleep, unconscious processing).
    Delta,
    /// Theta: 4-8 Hz (memory, navigation, meditation).
    Theta,
    /// Alpha: 8-13 Hz (relaxation, idling, inhibition).
    Alpha,
    /// Beta: 13-30 Hz (active thinking, focus, motor planning).
    Beta,
    /// Gamma: 30-100 Hz (binding, perception, consciousness).
    Gamma,
    /// High gamma: 100-200 Hz (cortical processing, fine motor).
    HighGamma,
    /// Custom frequency range.
    Custom {
        /// Lower bound in Hz.
        low_hz: f64,
        /// Upper bound in Hz.
        high_hz: f64,
    },
}

impl FrequencyBand {
    /// Returns the (low, high) frequency range in Hz.
    pub fn range_hz(&self) -> (f64, f64) {
        match self {
            FrequencyBand::Delta => (1.0, 4.0),
            FrequencyBand::Theta => (4.0, 8.0),
            FrequencyBand::Alpha => (8.0, 13.0),
            FrequencyBand::Beta => (13.0, 30.0),
            FrequencyBand::Gamma => (30.0, 100.0),
            FrequencyBand::HighGamma => (100.0, 200.0),
            FrequencyBand::Custom { low_hz, high_hz } => (*low_hz, *high_hz),
        }
    }

    /// Center frequency in Hz.
    pub fn center_hz(&self) -> f64 {
        let (lo, hi) = self.range_hz();
        (lo + hi) / 2.0
    }

    /// Bandwidth in Hz.
    pub fn bandwidth_hz(&self) -> f64 {
        let (lo, hi) = self.range_hz();
        hi - lo
    }
}

/// Spectral features for one channel at one time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralFeatures {
    /// Power in each frequency band.
    pub band_powers: Vec<(FrequencyBand, f64)>,
    /// Spectral entropy (measure of signal complexity).
    pub spectral_entropy: f64,
    /// Peak frequency in Hz.
    pub peak_frequency_hz: f64,
    /// Total power across all bands.
    pub total_power: f64,
}

/// Time-frequency representation (spectrogram-like).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeFrequencyMap {
    /// Data matrix: `data[time_window][frequency_bin]`.
    pub data: Vec<Vec<f64>>,
    /// Time points in seconds.
    pub time_points: Vec<f64>,
    /// Frequency bin centers in Hz.
    pub frequency_bins: Vec<f64>,
}

impl TimeFrequencyMap {
    /// Number of time windows.
    pub fn num_time_points(&self) -> usize {
        self.time_points.len()
    }

    /// Number of frequency bins.
    pub fn num_frequency_bins(&self) -> usize {
        self.frequency_bins.len()
    }
}

//! Mel spectrogram computation for audio feature extraction.
//!
//! This module provides efficient spectrogram computation using FFT
//! and mel-scale filterbanks, producing features suitable for ML models.

use ndarray::{Array2, Axis};
use rayon::prelude::*;
use realfft::RealFftPlanner;
use std::f32::consts::PI;
use tracing::{debug, instrument};

use crate::AudioError;

/// Configuration for spectrogram computation.
#[derive(Debug, Clone)]
pub struct SpectrogramConfig {
    /// Number of mel frequency bands.
    pub n_mels: usize,
    /// FFT window size in samples.
    pub n_fft: usize,
    /// Hop size between frames in samples.
    pub hop_length: usize,
    /// Sample rate of the input audio.
    pub sample_rate: u32,
    /// Minimum frequency for mel filterbank (Hz).
    pub f_min: f32,
    /// Maximum frequency for mel filterbank (Hz).
    pub f_max: f32,
    /// Whether to apply log scaling.
    pub log_scale: bool,
    /// Reference value for dB conversion.
    pub ref_db: f32,
    /// Minimum value for log scaling (avoids log(0)).
    pub min_value: f32,
}

impl Default for SpectrogramConfig {
    fn default() -> Self {
        Self {
            n_mels: 128,
            n_fft: 2048,
            hop_length: 512,
            sample_rate: 32_000,
            f_min: 0.0,
            f_max: 16_000.0, // Nyquist for 32kHz
            log_scale: true,
            ref_db: 1.0,
            min_value: 1e-10,
        }
    }
}

impl SpectrogramConfig {
    /// Creates a config optimized for 5-second segments producing 500 frames.
    ///
    /// For 32kHz audio:
    /// - 5s = 160,000 samples
    /// - hop_length = 320 gives ~500 frames
    #[must_use]
    pub fn for_5s_segment() -> Self {
        Self {
            n_mels: 128,
            n_fft: 2048,
            hop_length: 320, // 160000 / 320 = 500 frames
            sample_rate: 32_000,
            f_min: 500.0,    // Filter out very low frequencies
            f_max: 15_000.0, // Most bird calls below 15kHz
            log_scale: true,
            ref_db: 1.0,
            min_value: 1e-10,
        }
    }

    /// Creates a config for variable-length audio.
    #[must_use]
    pub fn with_target_frames(target_frames: usize, duration_ms: u64, sample_rate: u32) -> Self {
        let total_samples = (duration_ms as usize * sample_rate as usize) / 1000;
        let hop_length = total_samples / target_frames;

        Self {
            hop_length: hop_length.max(1),
            sample_rate,
            ..Self::default()
        }
    }
}

/// A computed mel spectrogram.
#[derive(Debug, Clone)]
pub struct MelSpectrogram {
    /// Spectrogram data (n_mels x n_frames).
    pub data: Array2<f32>,
    /// Configuration used to compute this spectrogram.
    pub config: SpectrogramConfig,
    /// Duration of the source audio in milliseconds.
    pub duration_ms: u64,
}

impl MelSpectrogram {
    /// Computes a mel spectrogram from audio samples.
    ///
    /// # Arguments
    /// * `samples` - Mono audio samples
    /// * `config` - Spectrogram configuration
    ///
    /// # Returns
    /// A MelSpectrogram with shape (n_mels, n_frames).
    #[instrument(skip(samples), fields(samples_len = samples.len()))]
    pub fn compute(samples: &[f32], config: SpectrogramConfig) -> Result<Self, AudioError> {
        if samples.is_empty() {
            return Err(AudioError::invalid_data("Cannot compute spectrogram of empty audio"));
        }

        let duration_ms = (samples.len() as u64 * 1000) / u64::from(config.sample_rate);

        // Compute STFT
        let stft = Self::stft(samples, config.n_fft, config.hop_length)?;

        // Compute mel filterbank
        let mel_filterbank = Self::create_mel_filterbank(
            config.n_mels,
            config.n_fft,
            config.sample_rate,
            config.f_min,
            config.f_max,
        );

        // Apply mel filterbank
        let n_frames = stft.ncols();
        let mut mel_spec = Array2::zeros((config.n_mels, n_frames));

        for (frame_idx, frame) in stft.axis_iter(Axis(1)).enumerate() {
            for (mel_idx, filter) in mel_filterbank.axis_iter(Axis(0)).enumerate() {
                let energy: f32 = frame
                    .iter()
                    .zip(filter.iter())
                    .map(|(s, f)| s * f)
                    .sum();
                mel_spec[[mel_idx, frame_idx]] = energy.max(config.min_value);
            }
        }

        // Apply log scaling if requested
        if config.log_scale {
            mel_spec.mapv_inplace(|x| 10.0 * (x / config.ref_db).log10());
        }

        debug!(
            n_mels = config.n_mels,
            n_frames = n_frames,
            duration_ms = duration_ms,
            "Spectrogram computed"
        );

        Ok(Self {
            data: mel_spec,
            config,
            duration_ms,
        })
    }

    /// Returns the shape as (n_mels, n_frames).
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        (self.data.nrows(), self.data.ncols())
    }

    /// Returns the number of mel bands.
    #[must_use]
    pub fn n_mels(&self) -> usize {
        self.data.nrows()
    }

    /// Returns the number of time frames.
    #[must_use]
    pub fn n_frames(&self) -> usize {
        self.data.ncols()
    }

    /// Extracts a time slice of the spectrogram.
    #[must_use]
    pub fn slice_frames(&self, start: usize, end: usize) -> Array2<f32> {
        let end = end.min(self.n_frames());
        let start = start.min(end);
        self.data.slice(ndarray::s![.., start..end]).to_owned()
    }

    /// Normalizes the spectrogram to zero mean and unit variance per mel band.
    pub fn normalize(&mut self) {
        for mut row in self.data.axis_iter_mut(Axis(0)) {
            let mean = row.mean().unwrap_or(0.0);
            let std = row.std(0.0);
            if std > 1e-6 {
                row.mapv_inplace(|x| (x - mean) / std);
            } else {
                row.mapv_inplace(|x| x - mean);
            }
        }
    }

    /// Returns the raw data as a flat vector (row-major order).
    #[must_use]
    pub fn to_vec(&self) -> Vec<f32> {
        self.data.iter().copied().collect()
    }

    /// Computes Short-Time Fourier Transform.
    fn stft(
        samples: &[f32],
        n_fft: usize,
        hop_length: usize,
    ) -> Result<Array2<f32>, AudioError> {
        let n_frames = (samples.len().saturating_sub(n_fft)) / hop_length + 1;
        if n_frames == 0 {
            return Err(AudioError::invalid_data(
                "Audio too short for FFT window size",
            ));
        }

        let n_bins = n_fft / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n_fft);

        // Pre-compute Hann window
        let window: Vec<f32> = (0..n_fft)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / n_fft as f32).cos()))
            .collect();

        // Compute STFT frames in parallel
        let frames: Vec<Vec<f32>> = (0..n_frames)
            .into_par_iter()
            .map(|frame_idx| {
                let start = frame_idx * hop_length;
                let mut input = vec![0.0f32; n_fft];

                // Copy and window the input
                for (i, &w) in window.iter().enumerate() {
                    if start + i < samples.len() {
                        input[i] = samples[start + i] * w;
                    }
                }

                // Perform FFT
                let mut spectrum = fft.make_output_vec();
                let mut scratch = fft.make_scratch_vec();

                // Clone fft for thread safety
                let fft = RealFftPlanner::<f32>::new().plan_fft_forward(n_fft);
                fft.process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                    .ok();

                // Compute magnitude spectrum
                spectrum
                    .iter()
                    .take(n_bins)
                    .map(|c| (c.re * c.re + c.im * c.im).sqrt())
                    .collect()
            })
            .collect();

        // Assemble into 2D array
        let mut stft = Array2::zeros((n_bins, n_frames));
        for (frame_idx, frame) in frames.into_iter().enumerate() {
            for (bin_idx, &value) in frame.iter().enumerate() {
                stft[[bin_idx, frame_idx]] = value;
            }
        }

        Ok(stft)
    }

    /// Creates a mel filterbank matrix.
    fn create_mel_filterbank(
        n_mels: usize,
        n_fft: usize,
        sample_rate: u32,
        f_min: f32,
        f_max: f32,
    ) -> Array2<f32> {
        let n_bins = n_fft / 2 + 1;

        // Convert frequency to mel scale
        let mel_min = Self::hz_to_mel(f_min);
        let mel_max = Self::hz_to_mel(f_max);

        // Create mel points equally spaced in mel scale
        let mel_points: Vec<f32> = (0..=n_mels + 1)
            .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32)
            .collect();

        // Convert back to Hz
        let hz_points: Vec<f32> = mel_points.iter().map(|&m| Self::mel_to_hz(m)).collect();

        // Convert to FFT bin indices
        let bin_points: Vec<usize> = hz_points
            .iter()
            .map(|&f| {
                let bin = (f * n_fft as f32 / sample_rate as f32).round() as usize;
                bin.min(n_bins - 1)
            })
            .collect();

        // Create filterbank matrix
        let mut filterbank = Array2::zeros((n_mels, n_bins));

        for m in 0..n_mels {
            let left = bin_points[m];
            let center = bin_points[m + 1];
            let right = bin_points[m + 2];

            // Rising slope
            for k in left..center {
                if center != left {
                    filterbank[[m, k]] = (k - left) as f32 / (center - left) as f32;
                }
            }

            // Falling slope
            for k in center..=right {
                if right != center {
                    filterbank[[m, k]] = (right - k) as f32 / (right - center) as f32;
                }
            }
        }

        filterbank
    }

    /// Converts frequency from Hz to mel scale.
    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    /// Converts frequency from mel scale to Hz.
    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10.0f32.powf(mel / 2595.0) - 1.0)
    }
}

/// Batch spectrogram computation for multiple segments.
pub struct SpectrogramBatch;

impl SpectrogramBatch {
    /// Computes spectrograms for multiple audio segments in parallel.
    pub fn compute_batch(
        segments: &[Vec<f32>],
        config: &SpectrogramConfig,
    ) -> Result<Vec<MelSpectrogram>, AudioError> {
        segments
            .par_iter()
            .map(|samples| MelSpectrogram::compute(samples, config.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_sine_wave(freq: f32, duration_s: f32, sample_rate: u32) -> Vec<f32> {
        let num_samples = (duration_s * sample_rate as f32) as usize;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * PI * freq * t).sin()
            })
            .collect()
    }

    #[test]
    fn test_spectrogram_config_default() {
        let config = SpectrogramConfig::default();
        assert_eq!(config.n_mels, 128);
        assert_eq!(config.n_fft, 2048);
    }

    #[test]
    fn test_spectrogram_5s_config() {
        let config = SpectrogramConfig::for_5s_segment();
        assert_eq!(config.hop_length, 320);
    }

    #[test]
    fn test_mel_conversion() {
        let hz = 1000.0;
        let mel = MelSpectrogram::hz_to_mel(hz);
        let hz_back = MelSpectrogram::mel_to_hz(mel);
        assert!((hz - hz_back).abs() < 0.01);
    }

    #[test]
    fn test_spectrogram_computation() {
        let samples = generate_sine_wave(1000.0, 1.0, 32000);
        let config = SpectrogramConfig::default();

        let spec = MelSpectrogram::compute(&samples, config).unwrap();

        assert_eq!(spec.n_mels(), 128);
        assert!(spec.n_frames() > 0);
    }

    #[test]
    fn test_spectrogram_5s_segment() {
        // 5 seconds at 32kHz = 160,000 samples
        let samples = generate_sine_wave(2000.0, 5.0, 32000);
        let config = SpectrogramConfig::for_5s_segment();

        let spec = MelSpectrogram::compute(&samples, config).unwrap();

        assert_eq!(spec.n_mels(), 128);
        // Should be approximately 500 frames
        assert!((spec.n_frames() as i32 - 500).abs() < 10);
    }

    #[test]
    fn test_spectrogram_normalization() {
        let samples = generate_sine_wave(1000.0, 1.0, 32000);
        let config = SpectrogramConfig::default();

        let mut spec = MelSpectrogram::compute(&samples, config).unwrap();
        spec.normalize();

        // Check that at least one row has roughly zero mean
        let first_row = spec.data.row(0);
        let mean = first_row.mean().unwrap_or(1.0);
        assert!(mean.abs() < 0.1);
    }

    #[test]
    fn test_spectrogram_slice() {
        let samples = generate_sine_wave(1000.0, 2.0, 32000);
        let config = SpectrogramConfig::default();

        let spec = MelSpectrogram::compute(&samples, config).unwrap();
        let slice = spec.slice_frames(0, 10);

        assert_eq!(slice.ncols(), 10);
        assert_eq!(slice.nrows(), spec.n_mels());
    }

    #[test]
    fn test_empty_input_error() {
        let config = SpectrogramConfig::default();
        let result = MelSpectrogram::compute(&[], config);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_computation() {
        let segment1 = generate_sine_wave(1000.0, 1.0, 32000);
        let segment2 = generate_sine_wave(2000.0, 1.0, 32000);
        let segments = vec![segment1, segment2];
        let config = SpectrogramConfig::default();

        let specs = SpectrogramBatch::compute_batch(&segments, &config).unwrap();

        assert_eq!(specs.len(), 2);
    }
}

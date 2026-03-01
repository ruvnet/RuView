//! Energy-based audio segmentation for isolating vocalizations.
//!
//! This module implements segmentation algorithms to detect regions
//! of interest (potential bird calls) in audio recordings based on
//! energy levels and signal characteristics.

use rayon::prelude::*;
use sevensense_core::RecordingId;
use tracing::{debug, instrument};

use super::AudioSegmenter;
use crate::domain::entities::{CallSegment, SignalQuality};
use crate::AudioError;

/// Energy-based audio segmenter.
///
/// Uses short-time energy analysis with adaptive thresholding
/// to detect regions containing vocalizations.
pub struct EnergySegmenter {
    config: SegmenterConfig,
}

/// Configuration for the energy segmenter.
#[derive(Debug, Clone)]
pub struct SegmenterConfig {
    /// Window size for energy calculation in samples.
    pub window_size: usize,
    /// Hop size between windows in samples.
    pub hop_size: usize,
    /// Minimum energy ratio above noise floor for detection.
    pub energy_threshold_ratio: f32,
    /// Minimum segment duration in milliseconds.
    pub min_segment_ms: u64,
    /// Maximum segment duration in milliseconds.
    pub max_segment_ms: u64,
    /// Minimum gap between segments in milliseconds.
    pub min_gap_ms: u64,
    /// Number of frames to use for noise floor estimation.
    pub noise_floor_frames: usize,
    /// Smoothing factor for energy envelope (0.0 to 1.0).
    pub smoothing: f32,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            window_size: 1024,      // ~32ms at 32kHz
            hop_size: 256,          // ~8ms hop
            energy_threshold_ratio: 3.0, // 3x noise floor
            min_segment_ms: 100,    // Minimum 100ms
            max_segment_ms: 10_000, // Maximum 10s
            min_gap_ms: 50,         // 50ms minimum gap
            noise_floor_frames: 10, // Use 10 quietest frames
            smoothing: 0.3,         // Light smoothing
        }
    }
}

impl EnergySegmenter {
    /// Creates a new EnergySegmenter with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SegmenterConfig::default(),
        }
    }

    /// Creates an EnergySegmenter with custom configuration.
    #[must_use]
    pub fn with_config(config: SegmenterConfig) -> Self {
        Self { config }
    }

    /// Calculates the RMS energy of a window.
    fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Calculates the peak amplitude in a window.
    fn calculate_peak(samples: &[f32]) -> f32 {
        samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, |a, b| a.max(b))
    }

    /// Calculates zero-crossing rate for a window.
    fn calculate_zcr(samples: &[f32]) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }
        let crossings: usize = samples
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        crossings as f32 / (samples.len() - 1) as f32
    }

    /// Estimates the noise floor from the quietest frames.
    fn estimate_noise_floor(&self, energies: &[f32]) -> f32 {
        let mut sorted = energies.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let noise_frames = self.config.noise_floor_frames.min(sorted.len() / 4).max(1);
        let noise_sum: f32 = sorted.iter().take(noise_frames).sum();
        (noise_sum / noise_frames as f32).max(1e-10)
    }

    /// Smooths the energy envelope using exponential moving average.
    fn smooth_envelope(&self, energies: &[f32]) -> Vec<f32> {
        let alpha = self.config.smoothing;
        let mut smoothed = Vec::with_capacity(energies.len());

        if energies.is_empty() {
            return smoothed;
        }

        smoothed.push(energies[0]);
        for &energy in &energies[1..] {
            let prev = *smoothed.last().unwrap();
            smoothed.push(alpha * energy + (1.0 - alpha) * prev);
        }

        smoothed
    }

    /// Finds segment boundaries from the binary activity signal.
    fn find_segments(
        &self,
        activity: &[bool],
        sample_rate: u32,
    ) -> Vec<(u64, u64)> {
        let hop_ms = (self.config.hop_size as u64 * 1000) / u64::from(sample_rate);
        let min_frames = (self.config.min_segment_ms / hop_ms).max(1) as usize;
        let max_frames = (self.config.max_segment_ms / hop_ms) as usize;
        let min_gap_frames = (self.config.min_gap_ms / hop_ms).max(1) as usize;

        let mut segments = Vec::new();
        let mut in_segment = false;
        let mut start_frame = 0;
        let mut gap_count = 0;

        for (i, &active) in activity.iter().enumerate() {
            if active {
                if !in_segment {
                    // Start new segment
                    start_frame = i;
                    in_segment = true;
                }
                gap_count = 0;
            } else if in_segment {
                gap_count += 1;
                if gap_count >= min_gap_frames {
                    // End segment
                    let end_frame = i - gap_count + 1;
                    let duration = end_frame - start_frame;

                    if duration >= min_frames && duration <= max_frames {
                        let start_ms = start_frame as u64 * hop_ms;
                        let end_ms = end_frame as u64 * hop_ms;
                        segments.push((start_ms, end_ms));
                    }

                    in_segment = false;
                    gap_count = 0;
                }
            }
        }

        // Handle segment at end of recording
        if in_segment {
            let end_frame = activity.len();
            let duration = end_frame - start_frame;

            if duration >= min_frames && duration <= max_frames {
                let start_ms = start_frame as u64 * hop_ms;
                let end_ms = end_frame as u64 * hop_ms;
                segments.push((start_ms, end_ms));
            }
        }

        segments
    }

    /// Assesses signal quality for a segment.
    fn assess_quality(
        &self,
        samples: &[f32],
        noise_floor: f32,
    ) -> (SignalQuality, f32, f32, f32) {
        let rms = Self::calculate_rms(samples);
        let peak = Self::calculate_peak(samples);
        let zcr = Self::calculate_zcr(samples);

        // Estimate SNR
        let snr_db = if noise_floor > 0.0 {
            20.0 * (rms / noise_floor).log10()
        } else {
            0.0
        };

        let quality = SignalQuality::from_metrics(snr_db, rms, zcr);

        (quality, peak, rms, zcr)
    }
}

impl Default for EnergySegmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSegmenter for EnergySegmenter {
    #[instrument(skip(self, samples), fields(samples_len = samples.len(), sample_rate = sample_rate))]
    fn segment(
        &self,
        samples: &[f32],
        sample_rate: u32,
        recording_id: RecordingId,
    ) -> Result<Vec<CallSegment>, AudioError> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let num_windows = (samples.len().saturating_sub(self.config.window_size))
            / self.config.hop_size
            + 1;

        if num_windows == 0 {
            return Ok(Vec::new());
        }

        debug!(num_windows = num_windows, "Starting energy analysis");

        // Calculate energy for each window (parallel)
        let energies: Vec<f32> = (0..num_windows)
            .into_par_iter()
            .map(|i| {
                let start = i * self.config.hop_size;
                let end = (start + self.config.window_size).min(samples.len());
                Self::calculate_rms(&samples[start..end])
            })
            .collect();

        // Smooth the energy envelope
        let smoothed = self.smooth_envelope(&energies);

        // Estimate noise floor
        let noise_floor = self.estimate_noise_floor(&smoothed);
        let threshold = noise_floor * self.config.energy_threshold_ratio;

        debug!(
            noise_floor = noise_floor,
            threshold = threshold,
            "Adaptive threshold calculated"
        );

        // Create binary activity signal
        let activity: Vec<bool> = smoothed.iter().map(|&e| e > threshold).collect();

        // Find segment boundaries
        let boundaries = self.find_segments(&activity, sample_rate);

        debug!(candidate_segments = boundaries.len(), "Found segment candidates");

        // Create CallSegment entities with quality assessment
        let segments: Vec<CallSegment> = boundaries
            .into_par_iter()
            .filter_map(|(start_ms, end_ms)| {
                let start_sample = (start_ms as usize * sample_rate as usize) / 1000;
                let end_sample = (end_ms as usize * sample_rate as usize) / 1000;
                let end_sample = end_sample.min(samples.len());

                if start_sample >= end_sample {
                    return None;
                }

                let segment_samples = &samples[start_sample..end_sample];
                let (quality, peak, rms, zcr) =
                    self.assess_quality(segment_samples, noise_floor);

                Some(
                    CallSegment::new(recording_id, start_ms, end_ms, peak, rms, quality)
                        .with_zero_crossing_rate(zcr),
                )
            })
            .collect();

        debug!(
            final_segments = segments.len(),
            high_quality = segments.iter().filter(|s| matches!(s.signal_quality, SignalQuality::High)).count(),
            "Segmentation complete"
        );

        Ok(segments)
    }
}

/// Spectral-based segmenter for more sophisticated detection.
///
/// Uses spectral features in addition to energy for detection.
pub struct SpectralSegmenter {
    energy_segmenter: EnergySegmenter,
    /// Frequency range of interest in Hz.
    freq_range: (f32, f32),
}

impl SpectralSegmenter {
    /// Creates a new SpectralSegmenter focused on bird frequencies.
    #[must_use]
    pub fn new() -> Self {
        Self {
            energy_segmenter: EnergySegmenter::new(),
            freq_range: (1000.0, 10000.0), // Bird vocalization range
        }
    }

    /// Sets the frequency range of interest.
    #[must_use]
    pub fn with_freq_range(mut self, min_hz: f32, max_hz: f32) -> Self {
        self.freq_range = (min_hz, max_hz);
        self
    }
}

impl Default for SpectralSegmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSegmenter for SpectralSegmenter {
    fn segment(
        &self,
        samples: &[f32],
        sample_rate: u32,
        recording_id: RecordingId,
    ) -> Result<Vec<CallSegment>, AudioError> {
        // For now, delegate to energy segmenter
        // Future: Add bandpass filtering for freq_range
        self.energy_segmenter
            .segment(samples, sample_rate, recording_id)
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
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect()
    }

    fn generate_test_signal(sample_rate: u32) -> Vec<f32> {
        let mut samples = Vec::new();

        // 1s silence
        samples.extend(vec![0.001f32; sample_rate as usize]);

        // 0.5s tone
        samples.extend(generate_sine_wave(1000.0, 0.5, sample_rate));

        // 0.3s silence
        samples.extend(vec![0.001f32; (sample_rate as f32 * 0.3) as usize]);

        // 0.8s tone
        samples.extend(generate_sine_wave(2000.0, 0.8, sample_rate));

        // 0.5s silence
        samples.extend(vec![0.001f32; (sample_rate as f32 * 0.5) as usize]);

        samples
    }

    #[test]
    fn test_rms_calculation() {
        let samples = vec![0.5, -0.5, 0.5, -0.5];
        let rms = EnergySegmenter::calculate_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_peak_calculation() {
        let samples = vec![0.3, -0.8, 0.5, -0.2];
        let peak = EnergySegmenter::calculate_peak(&samples);
        assert!((peak - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_zcr_calculation() {
        // Pure sine wave has high ZCR
        let sine: Vec<f32> = (0..100)
            .map(|i| (i as f32 * 0.5).sin())
            .collect();
        let zcr = EnergySegmenter::calculate_zcr(&sine);
        assert!(zcr > 0.0);
        assert!(zcr < 1.0);
    }

    #[test]
    fn test_segmentation() {
        let segmenter = EnergySegmenter::new();
        let samples = generate_test_signal(32000);
        let recording_id = RecordingId::new();

        let segments = segmenter.segment(&samples, 32000, recording_id).unwrap();

        // Should detect 2 segments
        assert_eq!(segments.len(), 2);

        // First segment should be around 1000-1500ms
        assert!(segments[0].start_ms >= 900 && segments[0].start_ms <= 1100);

        // Second segment should be around 1800-2600ms
        assert!(segments[1].start_ms >= 1700);
    }

    #[test]
    fn test_empty_input() {
        let segmenter = EnergySegmenter::new();
        let recording_id = RecordingId::new();

        let segments = segmenter.segment(&[], 32000, recording_id).unwrap();
        assert!(segments.is_empty());
    }

    #[test]
    fn test_silent_input() {
        let segmenter = EnergySegmenter::new();
        let recording_id = RecordingId::new();
        let silence = vec![0.0f32; 32000];

        let segments = segmenter.segment(&silence, 32000, recording_id).unwrap();
        assert!(segments.is_empty());
    }

    #[test]
    fn test_config_customization() {
        let config = SegmenterConfig {
            min_segment_ms: 200,
            max_segment_ms: 5000,
            energy_threshold_ratio: 2.0,
            ..Default::default()
        };

        let segmenter = EnergySegmenter::with_config(config);
        assert_eq!(segmenter.config.min_segment_ms, 200);
    }

    #[test]
    fn test_signal_quality_assessment() {
        // High quality signal (good SNR)
        let high_snr = vec![0.5f32; 1000];
        let noise_floor = 0.01;
        let segmenter = EnergySegmenter::new();
        let (quality, _, _, _) = segmenter.assess_quality(&high_snr, noise_floor);
        assert!(matches!(quality, SignalQuality::High | SignalQuality::Medium));

        // Low quality signal (low SNR)
        let low_snr = vec![0.02f32; 1000];
        let (quality, _, _, _) = segmenter.assess_quality(&low_snr, noise_floor);
        assert!(matches!(quality, SignalQuality::Low | SignalQuality::Noise | SignalQuality::Medium));
    }
}

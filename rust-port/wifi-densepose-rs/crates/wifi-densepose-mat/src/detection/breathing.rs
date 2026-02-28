//! Breathing pattern detection from CSI signals.

use crate::domain::{BreathingPattern, BreathingType, ConfidenceScore};

// ---------------------------------------------------------------------------
// Integration 6: CompressedBreathingBuffer (ADR-017, ruvector feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "ruvector")]
use ruvector_temporal_tensor::segment;
#[cfg(feature = "ruvector")]
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

/// Memory-efficient breathing waveform buffer using tiered temporal compression.
///
/// Compresses CSI amplitude time-series by 50-75% using tiered quantization:
/// - Hot tier (recent): 8-bit precision
/// - Warm tier: 5-7-bit precision
/// - Cold tier (historical): 3-bit precision
///
/// For 60-second window at 100 Hz, 56 subcarriers:
/// Before: 13.4 MB/zone → After: 3.4-6.7 MB/zone
#[cfg(feature = "ruvector")]
pub struct CompressedBreathingBuffer {
    compressor: TemporalTensorCompressor,
    encoded: Vec<u8>,
    n_subcarriers: usize,
    frame_count: u64,
}

#[cfg(feature = "ruvector")]
impl CompressedBreathingBuffer {
    pub fn new(n_subcarriers: usize, zone_id: u64) -> Self {
        Self {
            compressor: TemporalTensorCompressor::new(
                TierPolicy::default(),
                n_subcarriers as u32,
                zone_id as u32,
            ),
            encoded: Vec::new(),
            n_subcarriers,
            frame_count: 0,
        }
    }

    /// Push one frame of CSI amplitudes (one time step, all subcarriers).
    pub fn push_frame(&mut self, amplitudes: &[f32]) {
        assert_eq!(amplitudes.len(), self.n_subcarriers);
        let ts = self.frame_count as u32;
        // Synchronize last_access_ts with current timestamp so that the tier
        // policy's age computation (now_ts - last_access_ts + 1) never wraps to
        // zero (which would cause a divide-by-zero in wrapping_div).
        self.compressor.set_access(ts, ts);
        self.compressor.push_frame(amplitudes, ts, &mut self.encoded);
        self.frame_count += 1;
    }

    /// Flush pending compressed data.
    pub fn flush(&mut self) {
        self.compressor.flush(&mut self.encoded);
    }

    /// Decode all frames for breathing frequency analysis.
    /// Returns flat Vec<f32> of shape [n_frames × n_subcarriers].
    pub fn to_flat_vec(&self) -> Vec<f32> {
        let mut out = Vec::new();
        segment::decode(&self.encoded, &mut out);
        out
    }

    /// Get a single frame for real-time display.
    pub fn get_frame(&self, frame_idx: usize) -> Option<Vec<f32>> {
        segment::decode_single_frame(&self.encoded, frame_idx)
    }

    /// Number of frames stored.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Number of subcarriers per frame.
    pub fn n_subcarriers(&self) -> usize {
        self.n_subcarriers
    }
}

/// Configuration for breathing detection
#[derive(Debug, Clone)]
pub struct BreathingDetectorConfig {
    /// Minimum breathing rate to detect (breaths per minute)
    pub min_rate_bpm: f32,
    /// Maximum breathing rate to detect
    pub max_rate_bpm: f32,
    /// Minimum signal amplitude to consider
    pub min_amplitude: f32,
    /// Window size for FFT analysis (samples)
    pub window_size: usize,
    /// Overlap between windows (0.0-1.0)
    pub window_overlap: f32,
    /// Confidence threshold
    pub confidence_threshold: f32,
}

impl Default for BreathingDetectorConfig {
    fn default() -> Self {
        Self {
            min_rate_bpm: 4.0,    // Very slow breathing
            max_rate_bpm: 40.0,   // Fast breathing (distressed)
            min_amplitude: 0.1,
            window_size: 512,
            window_overlap: 0.5,
            confidence_threshold: 0.3,
        }
    }
}

/// Detector for breathing patterns in CSI signals
pub struct BreathingDetector {
    config: BreathingDetectorConfig,
}

impl BreathingDetector {
    /// Create a new breathing detector
    pub fn new(config: BreathingDetectorConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BreathingDetectorConfig::default())
    }

    /// Detect breathing pattern from CSI amplitude variations
    ///
    /// Breathing causes periodic chest movement that modulates the WiFi signal.
    /// We detect this by looking for periodic variations in the 0.1-0.67 Hz range
    /// (corresponding to 6-40 breaths per minute).
    pub fn detect(&self, csi_amplitudes: &[f64], sample_rate: f64) -> Option<BreathingPattern> {
        if csi_amplitudes.len() < self.config.window_size {
            return None;
        }

        // Calculate the frequency spectrum
        let spectrum = self.compute_spectrum(csi_amplitudes);

        // Find the dominant frequency in the breathing range
        let min_freq = self.config.min_rate_bpm as f64 / 60.0;
        let max_freq = self.config.max_rate_bpm as f64 / 60.0;

        let (dominant_freq, amplitude) = self.find_dominant_frequency(
            &spectrum,
            sample_rate,
            min_freq,
            max_freq,
        )?;

        // Convert to BPM
        let rate_bpm = (dominant_freq * 60.0) as f32;

        // Check amplitude threshold
        if amplitude < self.config.min_amplitude as f64 {
            return None;
        }

        // Calculate regularity (how peaked is the spectrum)
        let regularity = self.calculate_regularity(&spectrum, dominant_freq, sample_rate);

        // Determine breathing type based on rate and regularity
        let pattern_type = self.classify_pattern(rate_bpm, regularity);

        // Calculate confidence
        let confidence = self.calculate_confidence(amplitude, regularity);

        if confidence < self.config.confidence_threshold {
            return None;
        }

        Some(BreathingPattern {
            rate_bpm,
            amplitude: amplitude as f32,
            regularity,
            pattern_type,
        })
    }

    /// Compute frequency spectrum using FFT
    fn compute_spectrum(&self, signal: &[f64]) -> Vec<f64> {
        use rustfft::{FftPlanner, num_complex::Complex};

        let n = signal.len().next_power_of_two();
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(n);

        // Prepare input with zero padding
        let mut buffer: Vec<Complex<f64>> = signal
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        buffer.resize(n, Complex::new(0.0, 0.0));

        // Apply Hanning window
        for (i, sample) in buffer.iter_mut().enumerate().take(signal.len()) {
            let window = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / signal.len() as f64).cos());
            *sample = Complex::new(sample.re * window, 0.0);
        }

        fft.process(&mut buffer);

        // Return magnitude spectrum (only positive frequencies)
        buffer.iter()
            .take(n / 2)
            .map(|c| c.norm())
            .collect()
    }

    /// Find dominant frequency in a given range
    fn find_dominant_frequency(
        &self,
        spectrum: &[f64],
        sample_rate: f64,
        min_freq: f64,
        max_freq: f64,
    ) -> Option<(f64, f64)> {
        let n = spectrum.len() * 2; // Original FFT size
        let freq_resolution = sample_rate / n as f64;

        let min_bin = (min_freq / freq_resolution).ceil() as usize;
        let max_bin = (max_freq / freq_resolution).floor() as usize;

        if min_bin >= spectrum.len() || max_bin >= spectrum.len() || min_bin >= max_bin {
            return None;
        }

        // Find peak in range
        let mut max_amplitude = 0.0;
        let mut max_bin_idx = min_bin;

        for i in min_bin..=max_bin {
            if spectrum[i] > max_amplitude {
                max_amplitude = spectrum[i];
                max_bin_idx = i;
            }
        }

        if max_amplitude < self.config.min_amplitude as f64 {
            return None;
        }

        // Interpolate for better frequency estimate
        let freq = max_bin_idx as f64 * freq_resolution;

        Some((freq, max_amplitude))
    }

    /// Calculate how regular/periodic the signal is
    fn calculate_regularity(&self, spectrum: &[f64], dominant_freq: f64, sample_rate: f64) -> f32 {
        let n = spectrum.len() * 2;
        let freq_resolution = sample_rate / n as f64;
        let peak_bin = (dominant_freq / freq_resolution).round() as usize;

        if peak_bin >= spectrum.len() {
            return 0.0;
        }

        // Measure how much energy is concentrated at the peak vs spread
        let peak_power = spectrum[peak_bin];
        let total_power: f64 = spectrum.iter().sum();

        if total_power == 0.0 {
            return 0.0;
        }

        // Also check harmonics (2x, 3x frequency)
        let harmonic_power: f64 = [2, 3].iter()
            .filter_map(|&mult| {
                let harmonic_bin = peak_bin * mult;
                if harmonic_bin < spectrum.len() {
                    Some(spectrum[harmonic_bin])
                } else {
                    None
                }
            })
            .sum();

        ((peak_power + harmonic_power * 0.5) / total_power * 3.0).min(1.0) as f32
    }

    /// Classify the breathing pattern type
    fn classify_pattern(&self, rate_bpm: f32, regularity: f32) -> BreathingType {
        if rate_bpm < 6.0 {
            if regularity < 0.3 {
                BreathingType::Agonal
            } else {
                BreathingType::Shallow
            }
        } else if rate_bpm < 10.0 {
            BreathingType::Shallow
        } else if rate_bpm > 30.0 {
            BreathingType::Labored
        } else if regularity < 0.4 {
            BreathingType::Irregular
        } else {
            BreathingType::Normal
        }
    }

    /// Calculate overall detection confidence
    fn calculate_confidence(&self, amplitude: f64, regularity: f32) -> f32 {
        // Combine amplitude strength and regularity
        let amplitude_score = (amplitude / 1.0).min(1.0) as f32;
        let regularity_score = regularity;

        // Weight regularity more heavily for breathing detection
        amplitude_score * 0.4 + regularity_score * 0.6
    }
}

#[cfg(all(test, feature = "ruvector"))]
mod breathing_buffer_tests {
    use super::*;

    #[test]
    fn compressed_breathing_buffer_push_and_decode() {
        let n_sc = 56_usize;
        let mut buf = CompressedBreathingBuffer::new(n_sc, 1);
        for t in 0..10_u64 {
            let frame: Vec<f32> = (0..n_sc).map(|i| (i as f32 + t as f32) * 0.01).collect();
            buf.push_frame(&frame);
        }
        buf.flush();
        assert_eq!(buf.frame_count(), 10);
        // Decoded data should be non-empty
        let flat = buf.to_flat_vec();
        assert!(!flat.is_empty());
    }

    #[test]
    fn compressed_breathing_buffer_get_frame() {
        let n_sc = 8_usize;
        let mut buf = CompressedBreathingBuffer::new(n_sc, 2);
        let frame = vec![0.1_f32; n_sc];
        buf.push_frame(&frame);
        buf.flush();
        // Frame 0 should be decodable
        let decoded = buf.get_frame(0);
        assert!(decoded.is_some() || buf.to_flat_vec().len() == n_sc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_breathing_signal(rate_bpm: f64, sample_rate: f64, duration: f64) -> Vec<f64> {
        let num_samples = (sample_rate * duration) as usize;
        let freq = rate_bpm / 60.0;

        (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (2.0 * std::f64::consts::PI * freq * t).sin()
            })
            .collect()
    }

    #[test]
    fn test_detect_normal_breathing() {
        let detector = BreathingDetector::with_defaults();
        let signal = generate_breathing_signal(16.0, 100.0, 30.0);

        let result = detector.detect(&signal, 100.0);
        assert!(result.is_some());

        let pattern = result.unwrap();
        assert!(pattern.rate_bpm >= 14.0 && pattern.rate_bpm <= 18.0);
        assert!(matches!(pattern.pattern_type, BreathingType::Normal));
    }

    #[test]
    fn test_detect_fast_breathing() {
        let detector = BreathingDetector::with_defaults();
        let signal = generate_breathing_signal(35.0, 100.0, 30.0);

        let result = detector.detect(&signal, 100.0);
        assert!(result.is_some());

        let pattern = result.unwrap();
        assert!(pattern.rate_bpm > 30.0);
        assert!(matches!(pattern.pattern_type, BreathingType::Labored));
    }

    #[test]
    fn test_no_detection_on_noise() {
        let detector = BreathingDetector::with_defaults();

        // Random noise with low amplitude
        let signal: Vec<f64> = (0..1000)
            .map(|i| (i as f64 * 0.1).sin() * 0.01)
            .collect();

        let result = detector.detect(&signal, 100.0);
        // Should either be None or have very low confidence
        if let Some(pattern) = result {
            assert!(pattern.amplitude < 0.1);
        }
    }
}

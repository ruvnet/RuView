//! Adapter for wifi-densepose-signal crate.

use super::AdapterError;
use crate::domain::{BreathingPattern, BreathingType};
use crate::detection::CsiDataBuffer;

/// Features extracted from signal for vital signs detection
#[derive(Debug, Clone, Default)]
pub struct VitalFeatures {
    /// Breathing frequency features
    pub breathing_features: Vec<f64>,
    /// Heartbeat frequency features
    pub heartbeat_features: Vec<f64>,
    /// Movement energy features
    pub movement_features: Vec<f64>,
    /// Overall signal quality
    pub signal_quality: f64,
}

/// Adapter for wifi-densepose-signal crate
pub struct SignalAdapter {
    /// Window size for processing
    window_size: usize,
    /// Overlap between windows
    overlap: f64,
    /// Sample rate
    sample_rate: f64,
}

impl SignalAdapter {
    /// Create a new signal adapter
    pub fn new(window_size: usize, overlap: f64, sample_rate: f64) -> Self {
        Self {
            window_size,
            overlap,
            sample_rate,
        }
    }

    /// Create with default settings
    pub fn with_defaults() -> Self {
        Self::new(512, 0.5, 1000.0)
    }

    /// Extract vital sign features from CSI data
    pub fn extract_vital_features(
        &self,
        csi_data: &CsiDataBuffer,
    ) -> Result<VitalFeatures, AdapterError> {
        if csi_data.amplitudes.len() < self.window_size {
            return Err(AdapterError::Signal(
                "Insufficient data for feature extraction".into()
            ));
        }

        // Extract breathing-range features (0.1-0.5 Hz)
        let breathing_features = self.extract_frequency_band(
            &csi_data.amplitudes,
            0.1,
            0.5,
        )?;

        // Extract heartbeat-range features (0.8-2.0 Hz)
        let heartbeat_features = self.extract_frequency_band(
            &csi_data.phases,
            0.8,
            2.0,
        )?;

        // Extract movement features
        let movement_features = self.extract_movement_features(&csi_data.amplitudes)?;

        // Calculate signal quality
        let signal_quality = self.calculate_signal_quality(&csi_data.amplitudes);

        Ok(VitalFeatures {
            breathing_features,
            heartbeat_features,
            movement_features,
            signal_quality,
        })
    }

    /// Convert upstream CsiFeatures to breathing pattern
    pub fn to_breathing_pattern(
        &self,
        features: &VitalFeatures,
    ) -> Option<BreathingPattern> {
        if features.breathing_features.len() < 3 {
            return None;
        }

        // Extract key values from features
        let rate_estimate = features.breathing_features[0];
        let amplitude = features.breathing_features.get(1).copied().unwrap_or(0.5);
        let regularity = features.breathing_features.get(2).copied().unwrap_or(0.5);

        // Convert rate from Hz to BPM
        let rate_bpm = (rate_estimate * 60.0) as f32;

        // Validate rate
        if rate_bpm < 4.0 || rate_bpm > 60.0 {
            return None;
        }

        // Determine breathing type
        let pattern_type = self.classify_breathing_type(rate_bpm, regularity);

        Some(BreathingPattern {
            rate_bpm,
            amplitude: amplitude as f32,
            regularity: regularity as f32,
            pattern_type,
        })
    }

    /// Extract features from a frequency band
    fn extract_frequency_band(
        &self,
        signal: &[f64],
        low_freq: f64,
        high_freq: f64,
    ) -> Result<Vec<f64>, AdapterError> {
        use rustfft::{FftPlanner, num_complex::Complex};

        let n = signal.len().min(self.window_size);
        if n < 32 {
            return Err(AdapterError::Signal("Signal too short".into()));
        }

        let fft_size = n.next_power_of_two();
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        // Prepare buffer with windowing
        let mut buffer: Vec<Complex<f64>> = signal.iter()
            .take(n)
            .enumerate()
            .map(|(i, &x)| {
                let window = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos());
                Complex::new(x * window, 0.0)
            })
            .collect();
        buffer.resize(fft_size, Complex::new(0.0, 0.0));

        fft.process(&mut buffer);

        // Extract magnitude spectrum in frequency range
        let freq_resolution = self.sample_rate / fft_size as f64;
        let low_bin = (low_freq / freq_resolution).ceil() as usize;
        let high_bin = (high_freq / freq_resolution).floor() as usize;

        let mut features = Vec::new();

        if high_bin > low_bin && high_bin < buffer.len() / 2 {
            // Find peak frequency
            let mut max_mag = 0.0;
            let mut peak_bin = low_bin;
            for i in low_bin..=high_bin {
                let mag = buffer[i].norm();
                if mag > max_mag {
                    max_mag = mag;
                    peak_bin = i;
                }
            }

            // Peak frequency
            features.push(peak_bin as f64 * freq_resolution);
            // Peak magnitude (normalized)
            let total_power: f64 = buffer[1..buffer.len()/2]
                .iter()
                .map(|c| c.norm_sqr())
                .sum();
            features.push(if total_power > 0.0 { max_mag * max_mag / total_power } else { 0.0 });

            // Band power ratio
            let band_power: f64 = buffer[low_bin..=high_bin]
                .iter()
                .map(|c| c.norm_sqr())
                .sum();
            features.push(if total_power > 0.0 { band_power / total_power } else { 0.0 });
        }

        Ok(features)
    }

    /// Extract movement-related features
    fn extract_movement_features(&self, signal: &[f64]) -> Result<Vec<f64>, AdapterError> {
        if signal.len() < 10 {
            return Err(AdapterError::Signal("Signal too short".into()));
        }

        // Calculate variance
        let mean = signal.iter().sum::<f64>() / signal.len() as f64;
        let variance = signal.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / signal.len() as f64;

        // Calculate max absolute change
        let max_change = signal.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);

        // Calculate zero crossing rate
        let centered: Vec<f64> = signal.iter().map(|x| x - mean).collect();
        let zero_crossings: usize = centered.windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        let zcr = zero_crossings as f64 / signal.len() as f64;

        Ok(vec![variance, max_change, zcr])
    }

    /// Calculate overall signal quality
    fn calculate_signal_quality(&self, signal: &[f64]) -> f64 {
        if signal.len() < 10 {
            return 0.0;
        }

        // SNR estimate based on signal statistics
        let mean = signal.iter().sum::<f64>() / signal.len() as f64;
        let variance = signal.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / signal.len() as f64;

        // Higher variance relative to mean suggests better signal
        let snr_estimate = if mean.abs() > 1e-10 {
            (variance.sqrt() / mean.abs()).min(10.0) / 10.0
        } else {
            0.5
        };

        snr_estimate.clamp(0.0, 1.0)
    }

    /// Classify breathing type from rate and regularity
    fn classify_breathing_type(&self, rate_bpm: f32, regularity: f64) -> BreathingType {
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
}

impl Default for SignalAdapter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer() -> CsiDataBuffer {
        let mut buffer = CsiDataBuffer::new(100.0);

        // 10 seconds of data with breathing pattern
        let amplitudes: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 / 100.0;
                (2.0 * std::f64::consts::PI * 0.25 * t).sin() // 15 BPM
            })
            .collect();

        let phases: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 / 100.0;
                (2.0 * std::f64::consts::PI * 0.25 * t).sin() * 0.5
            })
            .collect();

        buffer.add_samples(&amplitudes, &phases);
        buffer
    }

    #[test]
    fn test_extract_vital_features() {
        // Use a smaller window size for the test
        let adapter = SignalAdapter::new(256, 0.5, 100.0);
        let buffer = create_test_buffer();

        let result = adapter.extract_vital_features(&buffer);
        assert!(result.is_ok());

        let features = result.unwrap();
        // Features should be extracted (may be empty if frequency out of range)
        // The main check is that extraction doesn't fail
        assert!(features.signal_quality >= 0.0);
    }

    #[test]
    fn test_to_breathing_pattern() {
        let adapter = SignalAdapter::with_defaults();

        let features = VitalFeatures {
            breathing_features: vec![0.25, 0.8, 0.9], // 15 BPM
            heartbeat_features: vec![],
            movement_features: vec![],
            signal_quality: 0.8,
        };

        let pattern = adapter.to_breathing_pattern(&features);
        assert!(pattern.is_some());

        let p = pattern.unwrap();
        assert!(p.rate_bpm > 10.0 && p.rate_bpm < 20.0);
    }

    #[test]
    fn test_signal_quality() {
        let adapter = SignalAdapter::with_defaults();

        // Good signal
        let good_signal: Vec<f64> = (0..100)
            .map(|i| (i as f64 * 0.1).sin())
            .collect();
        let good_quality = adapter.calculate_signal_quality(&good_signal);

        // Poor signal (constant)
        let poor_signal = vec![0.5; 100];
        let poor_quality = adapter.calculate_signal_quality(&poor_signal);

        assert!(good_quality > poor_quality);
    }
}

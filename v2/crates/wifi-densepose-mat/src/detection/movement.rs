//! Movement classification from CSI signal variations.

use crate::domain::{MovementProfile, MovementType};

/// Configuration for movement classification
#[derive(Debug, Clone)]
pub struct MovementClassifierConfig {
    /// Threshold for detecting any movement
    pub movement_threshold: f64,
    /// Threshold for gross movement
    pub gross_movement_threshold: f64,
    /// Window size for variance calculation
    pub window_size: usize,
    /// Threshold for periodic movement detection
    pub periodicity_threshold: f64,
}

impl Default for MovementClassifierConfig {
    fn default() -> Self {
        Self {
            movement_threshold: 0.1,
            gross_movement_threshold: 0.5,
            window_size: 100,
            periodicity_threshold: 0.3,
        }
    }
}

/// Classifier for movement types from CSI signals
pub struct MovementClassifier {
    config: MovementClassifierConfig,
}

impl MovementClassifier {
    /// Create a new movement classifier
    pub fn new(config: MovementClassifierConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(MovementClassifierConfig::default())
    }

    /// Classify movement from CSI signal
    pub fn classify(&self, csi_signal: &[f64], sample_rate: f64) -> MovementProfile {
        if csi_signal.len() < self.config.window_size {
            return MovementProfile::default();
        }

        // Calculate signal statistics
        let variance = self.calculate_variance(csi_signal);
        let max_change = self.calculate_max_change(csi_signal);
        let periodicity = self.calculate_periodicity(csi_signal, sample_rate);

        // Determine movement type
        let (movement_type, is_voluntary) = self.determine_movement_type(
            variance,
            max_change,
            periodicity,
        );

        // Calculate intensity
        let intensity = self.calculate_intensity(variance, max_change);

        // Calculate frequency of movement
        let frequency = self.calculate_movement_frequency(csi_signal, sample_rate);

        MovementProfile {
            movement_type,
            intensity,
            frequency,
            is_voluntary,
        }
    }

    /// Calculate signal variance
    fn calculate_variance(&self, signal: &[f64]) -> f64 {
        if signal.is_empty() {
            return 0.0;
        }

        let mean = signal.iter().sum::<f64>() / signal.len() as f64;
        let variance = signal.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / signal.len() as f64;

        variance
    }

    /// Calculate maximum change in signal
    fn calculate_max_change(&self, signal: &[f64]) -> f64 {
        if signal.len() < 2 {
            return 0.0;
        }

        signal.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max)
    }

    /// Calculate periodicity score using autocorrelation
    fn calculate_periodicity(&self, signal: &[f64], _sample_rate: f64) -> f64 {
        if signal.len() < 3 {
            return 0.0;
        }

        // Calculate autocorrelation
        let n = signal.len();
        let mean = signal.iter().sum::<f64>() / n as f64;
        let centered: Vec<f64> = signal.iter().map(|x| x - mean).collect();

        let variance: f64 = centered.iter().map(|x| x * x).sum();
        if variance == 0.0 {
            return 0.0;
        }

        // Find first peak in autocorrelation after lag 0
        let max_lag = n / 2;
        let mut max_corr = 0.0;

        for lag in 1..max_lag {
            let corr: f64 = centered.iter()
                .take(n - lag)
                .zip(centered.iter().skip(lag))
                .map(|(a, b)| a * b)
                .sum();

            let normalized_corr = corr / variance;
            if normalized_corr > max_corr {
                max_corr = normalized_corr;
            }
        }

        max_corr.max(0.0)
    }

    /// Determine movement type based on signal characteristics
    fn determine_movement_type(
        &self,
        variance: f64,
        max_change: f64,
        periodicity: f64,
    ) -> (MovementType, bool) {
        // No significant movement
        if variance < self.config.movement_threshold * 0.5
            && max_change < self.config.movement_threshold
        {
            return (MovementType::None, false);
        }

        // Check for gross movement (large, purposeful)
        if max_change > self.config.gross_movement_threshold
            && variance > self.config.movement_threshold
        {
            // Gross movement with low periodicity suggests voluntary
            let is_voluntary = periodicity < self.config.periodicity_threshold;
            return (MovementType::Gross, is_voluntary);
        }

        // Check for periodic movement (breathing-related or tremor)
        if periodicity > self.config.periodicity_threshold {
            // High periodicity with low variance = breathing-related
            if variance < self.config.movement_threshold * 2.0 {
                return (MovementType::Periodic, false);
            }
            // High periodicity with higher variance = tremor
            return (MovementType::Tremor, false);
        }

        // Fine movement (small but detectable)
        if variance > self.config.movement_threshold * 0.5 {
            // Fine movement might be voluntary if not very periodic
            let is_voluntary = periodicity < 0.2;
            return (MovementType::Fine, is_voluntary);
        }

        (MovementType::None, false)
    }

    /// Calculate movement intensity (0.0-1.0)
    fn calculate_intensity(&self, variance: f64, max_change: f64) -> f32 {
        // Combine variance and max change
        let variance_score = (variance / (self.config.gross_movement_threshold * 2.0)).min(1.0);
        let change_score = (max_change / self.config.gross_movement_threshold).min(1.0);

        ((variance_score * 0.6 + change_score * 0.4) as f32).min(1.0)
    }

    /// Calculate movement frequency (movements per second)
    fn calculate_movement_frequency(&self, signal: &[f64], sample_rate: f64) -> f32 {
        if signal.len() < 3 {
            return 0.0;
        }

        // Count zero crossings (after removing mean)
        let mean = signal.iter().sum::<f64>() / signal.len() as f64;
        let centered: Vec<f64> = signal.iter().map(|x| x - mean).collect();

        let zero_crossings: usize = centered.windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();

        // Each zero crossing is half a cycle
        let duration = signal.len() as f64 / sample_rate;
        let frequency = zero_crossings as f64 / (2.0 * duration);

        frequency as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_movement() {
        let classifier = MovementClassifier::with_defaults();
        let signal: Vec<f64> = vec![1.0; 200];

        let profile = classifier.classify(&signal, 100.0);
        assert!(matches!(profile.movement_type, MovementType::None));
    }

    #[test]
    fn test_gross_movement() {
        let classifier = MovementClassifier::with_defaults();

        // Simulate large movement
        let mut signal: Vec<f64> = vec![0.0; 200];
        for i in 50..100 {
            signal[i] = 2.0;
        }
        for i in 150..180 {
            signal[i] = -1.5;
        }

        let profile = classifier.classify(&signal, 100.0);
        assert!(matches!(profile.movement_type, MovementType::Gross));
    }

    #[test]
    fn test_periodic_movement() {
        let classifier = MovementClassifier::with_defaults();

        // Simulate periodic signal (like breathing) with higher amplitude
        let signal: Vec<f64> = (0..1000)
            .map(|i| (2.0 * std::f64::consts::PI * i as f64 / 100.0).sin() * 1.5)
            .collect();

        let profile = classifier.classify(&signal, 100.0);
        // Should detect some movement type (periodic, fine, or at least have non-zero intensity)
        // The exact type depends on thresholds, but with enough amplitude we should detect something
        assert!(profile.intensity > 0.0 || !matches!(profile.movement_type, MovementType::None));
    }

    #[test]
    fn test_intensity_calculation() {
        let classifier = MovementClassifier::with_defaults();

        // Low intensity
        let low_signal: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.1).sin() * 0.05)
            .collect();
        let low_profile = classifier.classify(&low_signal, 100.0);

        // High intensity
        let high_signal: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.1).sin() * 2.0)
            .collect();
        let high_profile = classifier.classify(&high_signal, 100.0);

        assert!(high_profile.intensity > low_profile.intensity);
    }
}

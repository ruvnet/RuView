//! Feature Extraction Module
//!
//! This module provides feature extraction capabilities for CSI data,
//! including amplitude, phase, correlation, Doppler, and power spectral density features.

use crate::csi_processor::CsiData;
use chrono::{DateTime, Utc};
use ndarray::{Array1, Array2};
use num_complex::Complex64;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

/// Amplitude-based features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmplitudeFeatures {
    /// Mean amplitude across antennas for each subcarrier
    pub mean: Array1<f64>,

    /// Variance of amplitude across antennas for each subcarrier
    pub variance: Array1<f64>,

    /// Peak amplitude value
    pub peak: f64,

    /// RMS amplitude
    pub rms: f64,

    /// Dynamic range (max - min)
    pub dynamic_range: f64,
}

impl AmplitudeFeatures {
    /// Extract amplitude features from CSI data
    pub fn from_csi_data(csi_data: &CsiData) -> Self {
        let amplitude = &csi_data.amplitude;
        let (nrows, ncols) = amplitude.dim();

        // Calculate mean across antennas (axis 0)
        let mut mean = Array1::zeros(ncols);
        for j in 0..ncols {
            let mut sum = 0.0;
            for i in 0..nrows {
                sum += amplitude[[i, j]];
            }
            mean[j] = sum / nrows as f64;
        }

        // Calculate variance across antennas
        let mut variance = Array1::zeros(ncols);
        for j in 0..ncols {
            let mut var_sum = 0.0;
            for i in 0..nrows {
                var_sum += (amplitude[[i, j]] - mean[j]).powi(2);
            }
            variance[j] = var_sum / nrows as f64;
        }

        // Calculate global statistics
        let flat: Vec<f64> = amplitude.iter().copied().collect();
        let peak = flat.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_val = flat.iter().cloned().fold(f64::INFINITY, f64::min);
        let dynamic_range = peak - min_val;

        let rms = (flat.iter().map(|x| x * x).sum::<f64>() / flat.len() as f64).sqrt();

        Self {
            mean,
            variance,
            peak,
            rms,
            dynamic_range,
        }
    }
}

/// Phase-based features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseFeatures {
    /// Phase differences between adjacent subcarriers (mean across antennas)
    pub difference: Array1<f64>,

    /// Phase variance across subcarriers
    pub variance: Array1<f64>,

    /// Phase gradient (rate of change)
    pub gradient: Array1<f64>,

    /// Phase coherence measure
    pub coherence: f64,
}

impl PhaseFeatures {
    /// Extract phase features from CSI data
    pub fn from_csi_data(csi_data: &CsiData) -> Self {
        let phase = &csi_data.phase;
        let (nrows, ncols) = phase.dim();

        // Calculate phase differences between adjacent subcarriers
        let mut diff_matrix = Array2::zeros((nrows, ncols.saturating_sub(1)));
        for i in 0..nrows {
            for j in 0..ncols.saturating_sub(1) {
                diff_matrix[[i, j]] = phase[[i, j + 1]] - phase[[i, j]];
            }
        }

        // Mean phase difference across antennas
        let mut difference = Array1::zeros(ncols.saturating_sub(1));
        for j in 0..ncols.saturating_sub(1) {
            let mut sum = 0.0;
            for i in 0..nrows {
                sum += diff_matrix[[i, j]];
            }
            difference[j] = sum / nrows as f64;
        }

        // Phase variance per subcarrier
        let mut variance = Array1::zeros(ncols);
        for j in 0..ncols {
            let mut col_sum = 0.0;
            for i in 0..nrows {
                col_sum += phase[[i, j]];
            }
            let mean = col_sum / nrows as f64;

            let mut var_sum = 0.0;
            for i in 0..nrows {
                var_sum += (phase[[i, j]] - mean).powi(2);
            }
            variance[j] = var_sum / nrows as f64;
        }

        // Calculate gradient (second order differences)
        let gradient = if ncols >= 3 {
            let mut grad = Array1::zeros(ncols.saturating_sub(2));
            for j in 0..ncols.saturating_sub(2) {
                grad[j] = difference[j + 1] - difference[j];
            }
            grad
        } else {
            Array1::zeros(1)
        };

        // Phase coherence (measure of phase stability)
        let coherence = Self::calculate_coherence(phase);

        Self {
            difference,
            variance,
            gradient,
            coherence,
        }
    }

    /// Calculate phase coherence
    fn calculate_coherence(phase: &Array2<f64>) -> f64 {
        let (nrows, ncols) = phase.dim();
        if nrows < 2 || ncols == 0 {
            return 0.0;
        }

        // Calculate coherence as the mean of cross-antenna phase correlation
        let mut coherence_sum = 0.0;
        let mut count = 0;

        for i in 0..nrows {
            for k in (i + 1)..nrows {
                // Calculate correlation between antenna pairs
                let row_i: Vec<f64> = phase.row(i).to_vec();
                let row_k: Vec<f64> = phase.row(k).to_vec();

                let mean_i: f64 = row_i.iter().sum::<f64>() / ncols as f64;
                let mean_k: f64 = row_k.iter().sum::<f64>() / ncols as f64;

                let mut cov = 0.0;
                let mut var_i = 0.0;
                let mut var_k = 0.0;

                for j in 0..ncols {
                    let diff_i = row_i[j] - mean_i;
                    let diff_k = row_k[j] - mean_k;
                    cov += diff_i * diff_k;
                    var_i += diff_i * diff_i;
                    var_k += diff_k * diff_k;
                }

                let std_prod = (var_i * var_k).sqrt();
                if std_prod > 1e-10 {
                    coherence_sum += cov / std_prod;
                    count += 1;
                }
            }
        }

        if count > 0 {
            coherence_sum / count as f64
        } else {
            0.0
        }
    }
}

/// Correlation features between antennas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationFeatures {
    /// Correlation matrix between antennas
    pub matrix: Array2<f64>,

    /// Mean off-diagonal correlation
    pub mean_correlation: f64,

    /// Maximum correlation coefficient
    pub max_correlation: f64,

    /// Correlation spread (std of off-diagonal elements)
    pub correlation_spread: f64,
}

impl CorrelationFeatures {
    /// Extract correlation features from CSI data
    pub fn from_csi_data(csi_data: &CsiData) -> Self {
        let amplitude = &csi_data.amplitude;
        let matrix = Self::correlation_matrix(amplitude);

        let (n, _) = matrix.dim();
        let mut off_diagonal: Vec<f64> = Vec::new();

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    off_diagonal.push(matrix[[i, j]]);
                }
            }
        }

        let mean_correlation = if !off_diagonal.is_empty() {
            off_diagonal.iter().sum::<f64>() / off_diagonal.len() as f64
        } else {
            0.0
        };

        let max_correlation = off_diagonal
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let correlation_spread = if !off_diagonal.is_empty() {
            let var: f64 = off_diagonal
                .iter()
                .map(|x| (x - mean_correlation).powi(2))
                .sum::<f64>()
                / off_diagonal.len() as f64;
            var.sqrt()
        } else {
            0.0
        };

        Self {
            matrix,
            mean_correlation,
            max_correlation: if max_correlation.is_finite() { max_correlation } else { 0.0 },
            correlation_spread,
        }
    }

    /// Compute correlation matrix between rows (antennas)
    fn correlation_matrix(data: &Array2<f64>) -> Array2<f64> {
        let (nrows, ncols) = data.dim();
        let mut corr = Array2::zeros((nrows, nrows));

        // Calculate means
        let means: Vec<f64> = (0..nrows)
            .map(|i| data.row(i).sum() / ncols as f64)
            .collect();

        // Calculate standard deviations
        let stds: Vec<f64> = (0..nrows)
            .map(|i| {
                let mean = means[i];
                let var: f64 = data.row(i).iter().map(|x| (x - mean).powi(2)).sum::<f64>() / ncols as f64;
                var.sqrt()
            })
            .collect();

        // Calculate correlation coefficients
        for i in 0..nrows {
            for j in 0..nrows {
                if i == j {
                    corr[[i, j]] = 1.0;
                } else {
                    let mut cov = 0.0;
                    for k in 0..ncols {
                        cov += (data[[i, k]] - means[i]) * (data[[j, k]] - means[j]);
                    }
                    cov /= ncols as f64;

                    let std_prod = stds[i] * stds[j];
                    corr[[i, j]] = if std_prod > 1e-10 { cov / std_prod } else { 0.0 };
                }
            }
        }

        corr
    }
}

/// Doppler shift features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopplerFeatures {
    /// Estimated Doppler shifts per subcarrier
    pub shifts: Array1<f64>,

    /// Peak Doppler frequency
    pub peak_frequency: f64,

    /// Mean Doppler shift magnitude
    pub mean_magnitude: f64,

    /// Doppler spread (standard deviation)
    pub spread: f64,
}

impl DopplerFeatures {
    /// Extract Doppler features from temporal CSI data
    pub fn from_csi_history(history: &[CsiData], sampling_rate: f64) -> Self {
        if history.is_empty() {
            return Self::empty();
        }

        let num_subcarriers = history[0].num_subcarriers;
        let num_samples = history.len();

        if num_samples < 2 {
            return Self::empty_with_size(num_subcarriers);
        }

        // Stack amplitude data for each subcarrier across time
        let mut shifts = Array1::zeros(num_subcarriers);
        let mut fft_planner = FftPlanner::new();
        let fft = fft_planner.plan_fft_forward(num_samples);

        for j in 0..num_subcarriers {
            // Extract time series for this subcarrier (use first antenna)
            let mut buffer: Vec<Complex64> = history
                .iter()
                .map(|csi| Complex64::new(csi.amplitude[[0, j]], 0.0))
                .collect();

            // Apply FFT
            fft.process(&mut buffer);

            // Find peak frequency (Doppler shift)
            let mut max_mag = 0.0;
            let mut max_idx = 0;

            for (idx, val) in buffer.iter().enumerate() {
                let mag = val.norm();
                if mag > max_mag && idx != 0 {
                    // Skip DC component
                    max_mag = mag;
                    max_idx = idx;
                }
            }

            // Convert bin index to frequency
            let freq_resolution = sampling_rate / num_samples as f64;
            let doppler_freq = if max_idx <= num_samples / 2 {
                max_idx as f64 * freq_resolution
            } else {
                (max_idx as i64 - num_samples as i64) as f64 * freq_resolution
            };

            shifts[j] = doppler_freq;
        }

        let magnitudes: Vec<f64> = shifts.iter().map(|x| x.abs()).collect();
        let peak_frequency = magnitudes.iter().cloned().fold(0.0, f64::max);
        let mean_magnitude = magnitudes.iter().sum::<f64>() / magnitudes.len() as f64;

        let spread = {
            let var: f64 = magnitudes
                .iter()
                .map(|x| (x - mean_magnitude).powi(2))
                .sum::<f64>()
                / magnitudes.len() as f64;
            var.sqrt()
        };

        Self {
            shifts,
            peak_frequency,
            mean_magnitude,
            spread,
        }
    }

    /// Create empty Doppler features
    fn empty() -> Self {
        Self {
            shifts: Array1::zeros(1),
            peak_frequency: 0.0,
            mean_magnitude: 0.0,
            spread: 0.0,
        }
    }

    /// Create empty Doppler features with specified size
    fn empty_with_size(size: usize) -> Self {
        Self {
            shifts: Array1::zeros(size),
            peak_frequency: 0.0,
            mean_magnitude: 0.0,
            spread: 0.0,
        }
    }
}

/// Power Spectral Density features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSpectralDensity {
    /// PSD values (frequency bins)
    pub values: Array1<f64>,

    /// Frequency bins in Hz
    pub frequencies: Array1<f64>,

    /// Total power
    pub total_power: f64,

    /// Peak power
    pub peak_power: f64,

    /// Peak frequency
    pub peak_frequency: f64,

    /// Spectral centroid
    pub centroid: f64,

    /// Spectral bandwidth
    pub bandwidth: f64,
}

impl PowerSpectralDensity {
    /// Calculate PSD from CSI amplitude data
    pub fn from_csi_data(csi_data: &CsiData, fft_size: usize) -> Self {
        let amplitude = &csi_data.amplitude;
        let flat: Vec<f64> = amplitude.iter().copied().collect();

        // Pad or truncate to FFT size
        let mut input: Vec<Complex64> = flat
            .iter()
            .take(fft_size)
            .map(|&x| Complex64::new(x, 0.0))
            .collect();

        while input.len() < fft_size {
            input.push(Complex64::new(0.0, 0.0));
        }

        // Apply FFT
        let mut fft_planner = FftPlanner::new();
        let fft = fft_planner.plan_fft_forward(fft_size);
        fft.process(&mut input);

        // Calculate power spectrum
        let mut psd = Array1::zeros(fft_size);
        for (i, val) in input.iter().enumerate() {
            psd[i] = val.norm_sqr() / fft_size as f64;
        }

        // Calculate frequency bins
        let freq_resolution = csi_data.bandwidth / fft_size as f64;
        let frequencies: Array1<f64> = (0..fft_size)
            .map(|i| {
                if i <= fft_size / 2 {
                    i as f64 * freq_resolution
                } else {
                    (i as i64 - fft_size as i64) as f64 * freq_resolution
                }
            })
            .collect();

        // Calculate statistics (use first half for positive frequencies)
        let half = fft_size / 2;
        let positive_psd: Vec<f64> = psd.iter().take(half).copied().collect();
        let positive_freq: Vec<f64> = frequencies.iter().take(half).copied().collect();

        let total_power: f64 = positive_psd.iter().sum();
        let peak_power = positive_psd.iter().cloned().fold(0.0, f64::max);

        let peak_idx = positive_psd
            .iter()
            .enumerate()
            .max_by(|(_, a): &(usize, &f64), (_, b): &(usize, &f64)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let peak_frequency = positive_freq[peak_idx];

        // Spectral centroid
        let centroid = if total_power > 1e-10 {
            let weighted_sum: f64 = positive_psd
                .iter()
                .zip(positive_freq.iter())
                .map(|(p, f)| p * f)
                .sum();
            weighted_sum / total_power
        } else {
            0.0
        };

        // Spectral bandwidth (standard deviation around centroid)
        let bandwidth = if total_power > 1e-10 {
            let weighted_var: f64 = positive_psd
                .iter()
                .zip(positive_freq.iter())
                .map(|(p, f)| p * (f - centroid).powi(2))
                .sum();
            (weighted_var / total_power).sqrt()
        } else {
            0.0
        };

        Self {
            values: psd,
            frequencies,
            total_power,
            peak_power,
            peak_frequency,
            centroid,
            bandwidth,
        }
    }
}

/// Complete CSI features collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiFeatures {
    /// Amplitude-based features
    pub amplitude: AmplitudeFeatures,

    /// Phase-based features
    pub phase: PhaseFeatures,

    /// Correlation features
    pub correlation: CorrelationFeatures,

    /// Doppler features (optional, requires history)
    pub doppler: Option<DopplerFeatures>,

    /// Power spectral density
    pub psd: PowerSpectralDensity,

    /// Timestamp of feature extraction
    pub timestamp: DateTime<Utc>,

    /// Source CSI metadata
    pub metadata: FeatureMetadata,
}

/// Metadata for extracted features
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureMetadata {
    /// Number of antennas in source data
    pub num_antennas: usize,

    /// Number of subcarriers in source data
    pub num_subcarriers: usize,

    /// FFT size used for PSD
    pub fft_size: usize,

    /// Sampling rate used for Doppler
    pub sampling_rate: Option<f64>,

    /// Number of samples used for Doppler
    pub doppler_samples: Option<usize>,
}

/// Configuration for feature extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureExtractorConfig {
    /// FFT size for PSD calculation
    pub fft_size: usize,

    /// Sampling rate for Doppler calculation
    pub sampling_rate: f64,

    /// Minimum history length for Doppler features
    pub min_doppler_history: usize,

    /// Enable Doppler feature extraction
    pub enable_doppler: bool,
}

impl Default for FeatureExtractorConfig {
    fn default() -> Self {
        Self {
            fft_size: 128,
            sampling_rate: 1000.0,
            min_doppler_history: 10,
            enable_doppler: true,
        }
    }
}

/// Feature extractor for CSI data
#[derive(Debug)]
pub struct FeatureExtractor {
    config: FeatureExtractorConfig,
}

impl FeatureExtractor {
    /// Create a new feature extractor
    pub fn new(config: FeatureExtractorConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(FeatureExtractorConfig::default())
    }

    /// Get configuration
    pub fn config(&self) -> &FeatureExtractorConfig {
        &self.config
    }

    /// Extract features from single CSI sample
    pub fn extract(&self, csi_data: &CsiData) -> CsiFeatures {
        let amplitude = AmplitudeFeatures::from_csi_data(csi_data);
        let phase = PhaseFeatures::from_csi_data(csi_data);
        let correlation = CorrelationFeatures::from_csi_data(csi_data);
        let psd = PowerSpectralDensity::from_csi_data(csi_data, self.config.fft_size);

        let metadata = FeatureMetadata {
            num_antennas: csi_data.num_antennas,
            num_subcarriers: csi_data.num_subcarriers,
            fft_size: self.config.fft_size,
            sampling_rate: None,
            doppler_samples: None,
        };

        CsiFeatures {
            amplitude,
            phase,
            correlation,
            doppler: None,
            psd,
            timestamp: Utc::now(),
            metadata,
        }
    }

    /// Extract features including Doppler from CSI history
    pub fn extract_with_history(&self, csi_data: &CsiData, history: &[CsiData]) -> CsiFeatures {
        let mut features = self.extract(csi_data);

        if self.config.enable_doppler && history.len() >= self.config.min_doppler_history {
            let doppler = DopplerFeatures::from_csi_history(history, self.config.sampling_rate);
            features.doppler = Some(doppler);
            features.metadata.sampling_rate = Some(self.config.sampling_rate);
            features.metadata.doppler_samples = Some(history.len());
        }

        features
    }

    /// Extract amplitude features only
    pub fn extract_amplitude(&self, csi_data: &CsiData) -> AmplitudeFeatures {
        AmplitudeFeatures::from_csi_data(csi_data)
    }

    /// Extract phase features only
    pub fn extract_phase(&self, csi_data: &CsiData) -> PhaseFeatures {
        PhaseFeatures::from_csi_data(csi_data)
    }

    /// Extract correlation features only
    pub fn extract_correlation(&self, csi_data: &CsiData) -> CorrelationFeatures {
        CorrelationFeatures::from_csi_data(csi_data)
    }

    /// Extract PSD features only
    pub fn extract_psd(&self, csi_data: &CsiData) -> PowerSpectralDensity {
        PowerSpectralDensity::from_csi_data(csi_data, self.config.fft_size)
    }

    /// Extract Doppler features from history
    pub fn extract_doppler(&self, history: &[CsiData]) -> Option<DopplerFeatures> {
        if history.len() >= self.config.min_doppler_history {
            Some(DopplerFeatures::from_csi_history(
                history,
                self.config.sampling_rate,
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    fn create_test_csi_data() -> CsiData {
        let amplitude = Array2::from_shape_fn((4, 64), |(i, j)| {
            1.0 + 0.5 * ((i + j) as f64 * 0.1).sin()
        });
        let phase = Array2::from_shape_fn((4, 64), |(i, j)| {
            0.5 * ((i + j) as f64 * 0.15).sin()
        });

        CsiData::builder()
            .amplitude(amplitude)
            .phase(phase)
            .frequency(5.0e9)
            .bandwidth(20.0e6)
            .snr(25.0)
            .build()
            .unwrap()
    }

    fn create_test_history(n: usize) -> Vec<CsiData> {
        (0..n)
            .map(|t| {
                let amplitude = Array2::from_shape_fn((4, 64), |(i, j)| {
                    1.0 + 0.3 * ((i + j + t) as f64 * 0.1).sin()
                });
                let phase = Array2::from_shape_fn((4, 64), |(i, j)| {
                    0.4 * ((i + j + t) as f64 * 0.12).sin()
                });

                CsiData::builder()
                    .amplitude(amplitude)
                    .phase(phase)
                    .frequency(5.0e9)
                    .bandwidth(20.0e6)
                    .build()
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn test_amplitude_features() {
        let csi_data = create_test_csi_data();
        let features = AmplitudeFeatures::from_csi_data(&csi_data);

        assert_eq!(features.mean.len(), 64);
        assert_eq!(features.variance.len(), 64);
        assert!(features.peak > 0.0);
        assert!(features.rms > 0.0);
        assert!(features.dynamic_range >= 0.0);
    }

    #[test]
    fn test_phase_features() {
        let csi_data = create_test_csi_data();
        let features = PhaseFeatures::from_csi_data(&csi_data);

        assert_eq!(features.difference.len(), 63);
        assert_eq!(features.variance.len(), 64);
        assert!(features.coherence.abs() <= 1.0);
    }

    #[test]
    fn test_correlation_features() {
        let csi_data = create_test_csi_data();
        let features = CorrelationFeatures::from_csi_data(&csi_data);

        assert_eq!(features.matrix.dim(), (4, 4));

        // Diagonal should be 1
        for i in 0..4 {
            assert!((features.matrix[[i, i]] - 1.0).abs() < 1e-10);
        }

        // Matrix should be symmetric
        for i in 0..4 {
            for j in 0..4 {
                assert!((features.matrix[[i, j]] - features.matrix[[j, i]]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_psd_features() {
        let csi_data = create_test_csi_data();
        let psd = PowerSpectralDensity::from_csi_data(&csi_data, 128);

        assert_eq!(psd.values.len(), 128);
        assert_eq!(psd.frequencies.len(), 128);
        assert!(psd.total_power >= 0.0);
        assert!(psd.peak_power >= 0.0);
    }

    #[test]
    fn test_doppler_features() {
        let history = create_test_history(20);
        let features = DopplerFeatures::from_csi_history(&history, 1000.0);

        assert_eq!(features.shifts.len(), 64);
    }

    #[test]
    fn test_feature_extractor() {
        let config = FeatureExtractorConfig::default();
        let extractor = FeatureExtractor::new(config);
        let csi_data = create_test_csi_data();

        let features = extractor.extract(&csi_data);

        assert_eq!(features.amplitude.mean.len(), 64);
        assert_eq!(features.phase.difference.len(), 63);
        assert_eq!(features.correlation.matrix.dim(), (4, 4));
        assert!(features.doppler.is_none());
    }

    #[test]
    fn test_feature_extractor_with_history() {
        let config = FeatureExtractorConfig {
            min_doppler_history: 10,
            enable_doppler: true,
            ..Default::default()
        };
        let extractor = FeatureExtractor::new(config);
        let csi_data = create_test_csi_data();
        let history = create_test_history(15);

        let features = extractor.extract_with_history(&csi_data, &history);

        assert!(features.doppler.is_some());
        assert_eq!(features.metadata.doppler_samples, Some(15));
    }

    #[test]
    fn test_individual_extraction() {
        let extractor = FeatureExtractor::default_config();
        let csi_data = create_test_csi_data();

        let amp = extractor.extract_amplitude(&csi_data);
        assert!(!amp.mean.is_empty());

        let phase = extractor.extract_phase(&csi_data);
        assert!(!phase.difference.is_empty());

        let corr = extractor.extract_correlation(&csi_data);
        assert_eq!(corr.matrix.dim(), (4, 4));

        let psd = extractor.extract_psd(&csi_data);
        assert!(!psd.values.is_empty());
    }

    #[test]
    fn test_empty_doppler_history() {
        let extractor = FeatureExtractor::default_config();
        let history: Vec<CsiData> = vec![];

        let doppler = extractor.extract_doppler(&history);
        assert!(doppler.is_none());
    }

    #[test]
    fn test_insufficient_doppler_history() {
        let config = FeatureExtractorConfig {
            min_doppler_history: 10,
            ..Default::default()
        };
        let extractor = FeatureExtractor::new(config);
        let history = create_test_history(5);

        let doppler = extractor.extract_doppler(&history);
        assert!(doppler.is_none());
    }
}

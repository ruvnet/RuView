//! Hampel Filter for robust outlier detection and removal.
//!
//! Uses running median and MAD (Median Absolute Deviation) instead of
//! mean/std, making it resistant to up to 50% contamination — unlike
//! Z-score methods where outliers corrupt the mean and mask themselves.
//!
//! # References
//! - Hampel (1974), "The Influence Curve and its Role in Robust Estimation"
//! - Used in WiGest (SenSys 2015), WiDance (MobiCom 2017)

/// Configuration for the Hampel filter.
#[derive(Debug, Clone)]
pub struct HampelConfig {
    /// Half-window size (total window = 2*half_window + 1)
    pub half_window: usize,
    /// Threshold in units of estimated σ (typically 3.0)
    pub threshold: f64,
}

impl Default for HampelConfig {
    fn default() -> Self {
        Self {
            half_window: 3,
            threshold: 3.0,
        }
    }
}

/// Result of Hampel filtering.
#[derive(Debug, Clone)]
pub struct HampelResult {
    /// Filtered signal (outliers replaced with local median)
    pub filtered: Vec<f64>,
    /// Indices where outliers were detected
    pub outlier_indices: Vec<usize>,
    /// Local median values at each sample
    pub medians: Vec<f64>,
    /// Estimated local σ at each sample
    pub sigma_estimates: Vec<f64>,
}

/// Scale factor converting MAD to σ for Gaussian distributions.
/// MAD = 0.6745 * σ → σ = MAD / 0.6745 = 1.4826 * MAD
const MAD_SCALE: f64 = 1.4826;

/// Apply Hampel filter to a 1D signal.
///
/// For each sample, computes the median and MAD of the surrounding window.
/// If the sample deviates from the median by more than `threshold * σ_est`,
/// it is replaced with the median.
pub fn hampel_filter(signal: &[f64], config: &HampelConfig) -> Result<HampelResult, HampelError> {
    if signal.is_empty() {
        return Err(HampelError::EmptySignal);
    }
    if config.half_window == 0 {
        return Err(HampelError::InvalidWindow);
    }

    let n = signal.len();
    let mut filtered = signal.to_vec();
    let mut outlier_indices = Vec::new();
    let mut medians = Vec::with_capacity(n);
    let mut sigma_estimates = Vec::with_capacity(n);

    for i in 0..n {
        let start = i.saturating_sub(config.half_window);
        let end = (i + config.half_window + 1).min(n);
        let window: Vec<f64> = signal[start..end].to_vec();

        let med = median(&window);
        let mad = median_absolute_deviation(&window, med);
        let sigma = MAD_SCALE * mad;

        medians.push(med);
        sigma_estimates.push(sigma);

        let deviation = (signal[i] - med).abs();
        let is_outlier = if sigma > 1e-15 {
            // Normal case: compare deviation to threshold * sigma
            deviation > config.threshold * sigma
        } else {
            // Zero-MAD case: all window values identical except possibly this sample.
            // Any non-zero deviation from the median is an outlier.
            deviation > 1e-15
        };

        if is_outlier {
            filtered[i] = med;
            outlier_indices.push(i);
        }
    }

    Ok(HampelResult {
        filtered,
        outlier_indices,
        medians,
        sigma_estimates,
    })
}

/// Apply Hampel filter to each row of a 2D array (e.g., per-antenna CSI).
pub fn hampel_filter_2d(
    data: &[Vec<f64>],
    config: &HampelConfig,
) -> Result<Vec<HampelResult>, HampelError> {
    data.iter().map(|row| hampel_filter(row, config)).collect()
}

/// Compute median of a slice (sorts a copy).
fn median(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Compute MAD (Median Absolute Deviation) given precomputed median.
fn median_absolute_deviation(data: &[f64], med: f64) -> f64 {
    let deviations: Vec<f64> = data.iter().map(|x| (x - med).abs()).collect();
    median(&deviations)
}

/// Errors from Hampel filtering.
#[derive(Debug, thiserror::Error)]
pub enum HampelError {
    #[error("Signal is empty")]
    EmptySignal,
    #[error("Half-window must be > 0")]
    InvalidWindow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_signal_unchanged() {
        // A smooth sinusoid should have zero outliers
        let signal: Vec<f64> = (0..100)
            .map(|i| (i as f64 * 0.1).sin())
            .collect();

        let result = hampel_filter(&signal, &HampelConfig::default()).unwrap();
        assert!(result.outlier_indices.is_empty());

        for i in 0..signal.len() {
            assert!(
                (result.filtered[i] - signal[i]).abs() < 1e-10,
                "Clean signal modified at index {}",
                i
            );
        }
    }

    #[test]
    fn test_single_spike_detected() {
        let mut signal: Vec<f64> = vec![1.0; 50];
        signal[25] = 100.0; // Huge spike

        let result = hampel_filter(&signal, &HampelConfig::default()).unwrap();
        assert!(result.outlier_indices.contains(&25));
        assert!((result.filtered[25] - 1.0).abs() < 1e-10); // Replaced with median
    }

    #[test]
    fn test_multiple_spikes() {
        let mut signal: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.05).sin())
            .collect();

        // Insert spikes
        signal[30] = 50.0;
        signal[100] = -50.0;
        signal[170] = 80.0;

        let config = HampelConfig {
            half_window: 5,
            threshold: 3.0,
        };
        let result = hampel_filter(&signal, &config).unwrap();

        assert!(result.outlier_indices.contains(&30));
        assert!(result.outlier_indices.contains(&100));
        assert!(result.outlier_indices.contains(&170));
    }

    #[test]
    fn test_z_score_masking_resistance() {
        // 50 clean samples + many outliers: Z-score would fail, Hampel should work
        let mut signal: Vec<f64> = vec![0.0; 100];
        // Insert 30% contamination (Z-score would be confused)
        for i in (0..100).step_by(3) {
            signal[i] = 50.0;
        }

        let config = HampelConfig {
            half_window: 5,
            threshold: 3.0,
        };
        let result = hampel_filter(&signal, &config).unwrap();

        // The contaminated samples should be detected as outliers
        assert!(!result.outlier_indices.is_empty());
    }

    #[test]
    fn test_2d_filtering() {
        let rows = vec![
            vec![1.0, 1.0, 100.0, 1.0, 1.0, 1.0, 1.0],
            vec![2.0, 2.0, 2.0, 2.0, -80.0, 2.0, 2.0],
        ];

        let results = hampel_filter_2d(&rows, &HampelConfig::default()).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].outlier_indices.contains(&2));
        assert!(results[1].outlier_indices.contains(&4));
    }

    #[test]
    fn test_median_computation() {
        assert!((median(&[1.0, 3.0, 2.0]) - 2.0).abs() < 1e-10);
        assert!((median(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 1e-10);
        assert!((median(&[5.0]) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_signal_error() {
        assert!(matches!(
            hampel_filter(&[], &HampelConfig::default()),
            Err(HampelError::EmptySignal)
        ));
    }
}

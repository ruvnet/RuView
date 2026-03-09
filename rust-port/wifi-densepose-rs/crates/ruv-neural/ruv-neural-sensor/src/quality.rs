//! Signal quality monitoring for neural sensor channels.

/// Signal quality metrics for a single channel.
pub struct SignalQuality {
    /// Signal-to-noise ratio in dB.
    pub snr_db: f64,
    /// Probability of artifact contamination in [0, 1].
    pub artifact_probability: f64,
    /// Whether the channel is saturated (clipping).
    pub saturated: bool,
}

impl SignalQuality {
    /// Returns true if signal quality is below acceptable thresholds.
    ///
    /// Thresholds: SNR < 3 dB or artifact_probability > 0.5.
    pub fn below_threshold(&self) -> bool {
        self.snr_db < 3.0 || self.artifact_probability > 0.5
    }
}

/// Real-time signal quality monitor for multi-channel data.
pub struct QualityMonitor {
    num_channels: usize,
}

impl QualityMonitor {
    /// Create a new quality monitor for the given number of channels.
    pub fn new(num_channels: usize) -> Self {
        Self { num_channels }
    }

    /// Check signal quality for each channel.
    ///
    /// Each element in `signals` is a slice of samples for one channel.
    pub fn check_quality(&mut self, signals: &[&[f64]]) -> Vec<SignalQuality> {
        let n = signals.len().min(self.num_channels);
        (0..n)
            .map(|i| {
                let signal = signals[i];
                let snr_db = estimate_snr_db(signal);
                let saturated = detect_saturation(signal);
                let artifact_probability = if saturated { 0.9 } else { 0.0 };
                SignalQuality {
                    snr_db,
                    artifact_probability,
                    saturated,
                }
            })
            .collect()
    }
}

/// Estimate SNR in dB from a signal segment.
fn estimate_snr_db(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let variance = signal.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / signal.len() as f64;
    let rms = variance.sqrt();
    if rms < 1e-15 {
        return 0.0;
    }
    let n = signal.len();
    if n < 4 {
        return 20.0 * rms.log10();
    }
    // Estimate noise as std of first differences (captures high-freq content).
    let diff_var = signal
        .windows(2)
        .map(|w| (w[1] - w[0]).powi(2))
        .sum::<f64>()
        / (n - 1) as f64;
    let noise_power = diff_var / 2.0;
    let signal_power = variance;
    if noise_power < 1e-15 {
        return 60.0;
    }
    10.0 * (signal_power / noise_power).log10()
}

/// Detect if a signal is saturated (extreme repeated values).
fn detect_saturation(signal: &[f64]) -> bool {
    if signal.len() < 10 {
        return false;
    }
    let max_abs = signal.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
    if max_abs < 1e-10 {
        return false;
    }
    let threshold = max_abs * 0.999;
    let clipped_count = signal.iter().filter(|x| x.abs() >= threshold).count();
    clipped_count as f64 / signal.len() as f64 > 0.1
}

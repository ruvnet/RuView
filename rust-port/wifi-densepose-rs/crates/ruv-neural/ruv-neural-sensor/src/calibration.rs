//! Sensor calibration utilities for gain/offset correction and cross-calibration.

/// Calibration data for a sensor array.
pub struct CalibrationData {
    /// Per-channel gain factors.
    pub gains: Vec<f64>,
    /// Per-channel DC offsets to subtract.
    pub offsets: Vec<f64>,
    /// Per-channel noise floor estimates (fT RMS).
    pub noise_floors: Vec<f64>,
}

/// Apply gain and offset correction to a single sample on a given channel.
///
/// `corrected = (raw - offset) * gain`
pub fn calibrate_channel(raw: f64, channel: usize, cal: &CalibrationData) -> f64 {
    let offset = cal.offsets.get(channel).copied().unwrap_or(0.0);
    let gain = cal.gains.get(channel).copied().unwrap_or(1.0);
    (raw - offset) * gain
}

/// Estimate the noise floor (RMS) of a quiet signal segment.
pub fn estimate_noise_floor(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let mean_sq = signal.iter().map(|x| x * x).sum::<f64>() / signal.len() as f64;
    mean_sq.sqrt()
}

/// Cross-calibrate a target channel against a reference channel.
///
/// Returns `(gain, offset)` such that `target * gain + offset ~ reference`.
/// Uses simple linear regression.
pub fn cross_calibrate(reference: &[f64], target: &[f64]) -> (f64, f64) {
    let n = reference.len().min(target.len());
    if n == 0 {
        return (1.0, 0.0);
    }

    let mean_r = reference[..n].iter().sum::<f64>() / n as f64;
    let mean_t = target[..n].iter().sum::<f64>() / n as f64;

    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..n {
        let dr = reference[i] - mean_r;
        let dt = target[i] - mean_t;
        num += dr * dt;
        den += dt * dt;
    }

    if den.abs() < 1e-15 {
        return (1.0, mean_r - mean_t);
    }

    let gain = num / den;
    let offset = mean_r - gain * mean_t;
    (gain, offset)
}

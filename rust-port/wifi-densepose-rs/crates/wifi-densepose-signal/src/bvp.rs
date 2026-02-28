//! Body Velocity Profile (BVP) extraction.
//!
//! BVP is a domain-independent 2D representation (velocity × time) that encodes
//! how different body parts move at different speeds. Because BVP captures
//! velocity distributions rather than raw CSI values, it generalizes across
//! environments (different rooms, furniture, AP placement).
//!
//! # Algorithm
//! 1. Apply STFT to each subcarrier's temporal amplitude stream
//! 2. Map frequency bins to velocity via v = f_doppler * λ / 2
//! 3. Aggregate |STFT| across subcarriers to form BVP
//!
//! # References
//! - Widar 3.0: Zero-Effort Cross-Domain Gesture Recognition (MobiSys 2019)

use ndarray::Array2;
use num_complex::Complex64;
use rustfft::FftPlanner;
use std::f64::consts::PI;

/// Configuration for BVP extraction.
#[derive(Debug, Clone)]
pub struct BvpConfig {
    /// STFT window size (samples)
    pub window_size: usize,
    /// STFT hop size (samples)
    pub hop_size: usize,
    /// Carrier frequency in Hz (for velocity mapping)
    pub carrier_frequency: f64,
    /// Number of velocity bins to output
    pub n_velocity_bins: usize,
    /// Maximum velocity to resolve (m/s)
    pub max_velocity: f64,
}

impl Default for BvpConfig {
    fn default() -> Self {
        Self {
            window_size: 128,
            hop_size: 32,
            carrier_frequency: 5.0e9,
            n_velocity_bins: 64,
            max_velocity: 2.0,
        }
    }
}

/// Body Velocity Profile result.
#[derive(Debug, Clone)]
pub struct BodyVelocityProfile {
    /// BVP matrix: (n_velocity_bins × n_time_frames)
    /// Each column is a velocity distribution at a time instant.
    pub data: Array2<f64>,
    /// Velocity values for each row bin (m/s)
    pub velocity_bins: Vec<f64>,
    /// Number of time frames
    pub n_time: usize,
    /// Time resolution (seconds per frame)
    pub time_resolution: f64,
    /// Velocity resolution (m/s per bin)
    pub velocity_resolution: f64,
}

/// Extract Body Velocity Profile from temporal CSI data.
///
/// `csi_temporal`: (num_samples × num_subcarriers) amplitude matrix
/// `sample_rate`: sampling rate in Hz
pub fn extract_bvp(
    csi_temporal: &Array2<f64>,
    sample_rate: f64,
    config: &BvpConfig,
) -> Result<BodyVelocityProfile, BvpError> {
    let (n_samples, n_sc) = csi_temporal.dim();

    if n_samples < config.window_size {
        return Err(BvpError::InsufficientSamples {
            needed: config.window_size,
            got: n_samples,
        });
    }
    if n_sc == 0 {
        return Err(BvpError::NoSubcarriers);
    }
    if config.hop_size == 0 || config.window_size == 0 {
        return Err(BvpError::InvalidConfig("window_size and hop_size must be > 0".into()));
    }

    let wavelength = 2.998e8 / config.carrier_frequency;
    let n_frames = (n_samples - config.window_size) / config.hop_size + 1;
    let n_fft_bins = config.window_size / 2 + 1;

    // Hann window
    let window: Vec<f64> = (0..config.window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (config.window_size - 1) as f64).cos()))
        .collect();

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(config.window_size);

    // Compute STFT magnitude for each subcarrier, then aggregate
    let mut aggregated = Array2::zeros((n_fft_bins, n_frames));

    for sc in 0..n_sc {
        let col: Vec<f64> = csi_temporal.column(sc).to_vec();

        // Remove DC from this subcarrier
        let mean: f64 = col.iter().sum::<f64>() / col.len() as f64;

        for frame in 0..n_frames {
            let start = frame * config.hop_size;

            let mut buffer: Vec<Complex64> = col[start..start + config.window_size]
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| Complex64::new((s - mean) * w, 0.0))
                .collect();

            fft.process(&mut buffer);

            // Accumulate magnitude across subcarriers
            for bin in 0..n_fft_bins {
                aggregated[[bin, frame]] += buffer[bin].norm();
            }
        }
    }

    // Normalize by number of subcarriers
    aggregated /= n_sc as f64;

    // Map FFT bins to velocity bins
    let freq_resolution = sample_rate / config.window_size as f64;
    let velocity_resolution = config.max_velocity * 2.0 / config.n_velocity_bins as f64;

    let velocity_bins: Vec<f64> = (0..config.n_velocity_bins)
        .map(|i| -config.max_velocity + i as f64 * velocity_resolution)
        .collect();

    // Resample FFT bins to velocity bins using v = f_doppler * λ / 2
    let mut bvp = Array2::zeros((config.n_velocity_bins, n_frames));

    for (v_idx, &velocity) in velocity_bins.iter().enumerate() {
        // Convert velocity to Doppler frequency
        let doppler_freq = 2.0 * velocity / wavelength;
        // Convert to FFT bin index
        let fft_bin = (doppler_freq.abs() / freq_resolution).round() as usize;

        if fft_bin < n_fft_bins {
            for frame in 0..n_frames {
                bvp[[v_idx, frame]] = aggregated[[fft_bin, frame]];
            }
        }
    }

    Ok(BodyVelocityProfile {
        data: bvp,
        velocity_bins,
        n_time: n_frames,
        time_resolution: config.hop_size as f64 / sample_rate,
        velocity_resolution,
    })
}

/// Errors from BVP extraction.
#[derive(Debug, thiserror::Error)]
pub enum BvpError {
    #[error("Insufficient samples: need {needed}, got {got}")]
    InsufficientSamples { needed: usize, got: usize },

    #[error("No subcarriers in input")]
    NoSubcarriers,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bvp_dimensions() {
        let n_samples = 1000;
        let n_sc = 10;
        let csi = Array2::from_shape_fn((n_samples, n_sc), |(t, sc)| {
            let freq = 1.0 + sc as f64 * 0.3;
            (2.0 * PI * freq * t as f64 / 100.0).sin()
        });

        let config = BvpConfig {
            window_size: 128,
            hop_size: 32,
            n_velocity_bins: 64,
            ..Default::default()
        };

        let bvp = extract_bvp(&csi, 100.0, &config).unwrap();
        assert_eq!(bvp.data.dim().0, 64); // velocity bins
        let expected_frames = (1000 - 128) / 32 + 1;
        assert_eq!(bvp.n_time, expected_frames);
        assert_eq!(bvp.velocity_bins.len(), 64);
    }

    #[test]
    fn test_bvp_velocity_range() {
        let csi = Array2::from_shape_fn((500, 5), |(t, _)| (t as f64 * 0.05).sin());

        let config = BvpConfig {
            max_velocity: 3.0,
            n_velocity_bins: 60,
            window_size: 64,
            hop_size: 16,
            ..Default::default()
        };

        let bvp = extract_bvp(&csi, 100.0, &config).unwrap();

        // Velocity bins should span [-3.0, +3.0)
        assert!(bvp.velocity_bins[0] < 0.0);
        assert!(*bvp.velocity_bins.last().unwrap() > 0.0);
        assert!((bvp.velocity_bins[0] - (-3.0)).abs() < 0.2);
    }

    #[test]
    fn test_static_scene_low_velocity() {
        // Constant signal → no Doppler → BVP should peak at velocity=0
        let csi = Array2::from_elem((500, 10), 1.0);

        let config = BvpConfig {
            window_size: 64,
            hop_size: 32,
            n_velocity_bins: 32,
            max_velocity: 1.0,
            ..Default::default()
        };

        let bvp = extract_bvp(&csi, 100.0, &config).unwrap();

        // After removing DC and applying window, constant signal has
        // near-zero energy at all Doppler frequencies
        let total_energy: f64 = bvp.data.iter().sum();
        // For a constant signal with DC removed, total energy should be very small
        assert!(
            total_energy < 1.0,
            "Static scene should have low Doppler energy, got {}",
            total_energy
        );
    }

    #[test]
    fn test_moving_body_nonzero_velocity() {
        // A sinusoidal amplitude modulation simulates motion → Doppler energy
        let n = 1000;
        let motion_freq = 5.0; // Hz
        let csi = Array2::from_shape_fn((n, 8), |(t, _)| {
            1.0 + 0.5 * (2.0 * PI * motion_freq * t as f64 / 100.0).sin()
        });

        let config = BvpConfig {
            window_size: 128,
            hop_size: 32,
            n_velocity_bins: 64,
            max_velocity: 2.0,
            ..Default::default()
        };

        let bvp = extract_bvp(&csi, 100.0, &config).unwrap();
        let total_energy: f64 = bvp.data.iter().sum();
        assert!(total_energy > 0.0, "Moving body should produce Doppler energy");
    }

    #[test]
    fn test_insufficient_samples() {
        let csi = Array2::from_elem((10, 5), 1.0);
        let config = BvpConfig {
            window_size: 128,
            ..Default::default()
        };
        assert!(matches!(
            extract_bvp(&csi, 100.0, &config),
            Err(BvpError::InsufficientSamples { .. })
        ));
    }

    #[test]
    fn test_time_resolution() {
        let csi = Array2::from_elem((500, 5), 1.0);
        let config = BvpConfig {
            window_size: 64,
            hop_size: 32,
            ..Default::default()
        };

        let bvp = extract_bvp(&csi, 100.0, &config).unwrap();
        assert!((bvp.time_resolution - 0.32).abs() < 1e-6); // 32/100
    }
}

//! CSI Spectrogram Generation
//!
//! Constructs 2D time-frequency matrices via Short-Time Fourier Transform (STFT)
//! applied to temporal CSI amplitude streams. The resulting spectrograms are the
//! standard input format for CNN-based WiFi activity recognition.
//!
//! # References
//! - Used in virtually all CNN-based WiFi sensing papers since 2018

use ndarray::Array2;
use num_complex::Complex64;
use ruvector_attn_mincut::attn_mincut;
use rustfft::FftPlanner;
use std::f64::consts::PI;

/// Configuration for spectrogram generation.
#[derive(Debug, Clone)]
pub struct SpectrogramConfig {
    /// FFT window size (number of samples per frame)
    pub window_size: usize,
    /// Hop size (step between consecutive frames). Smaller = more overlap.
    pub hop_size: usize,
    /// Window function to apply
    pub window_fn: WindowFunction,
    /// Whether to compute power (magnitude squared) or magnitude
    pub power: bool,
}

impl Default for SpectrogramConfig {
    fn default() -> Self {
        Self {
            window_size: 256,
            hop_size: 64,
            window_fn: WindowFunction::Hann,
            power: true,
        }
    }
}

/// Window function types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFunction {
    /// Rectangular (no windowing)
    Rectangular,
    /// Hann window (cosine-squared taper)
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window (lower sidelobe level)
    Blackman,
}

/// Result of spectrogram computation.
#[derive(Debug, Clone)]
pub struct Spectrogram {
    /// Power/magnitude values: rows = frequency bins, columns = time frames.
    /// Only positive frequencies (0 to Nyquist), so rows = window_size/2 + 1.
    pub data: Array2<f64>,
    /// Number of frequency bins
    pub n_freq: usize,
    /// Number of time frames
    pub n_time: usize,
    /// Frequency resolution (Hz per bin)
    pub freq_resolution: f64,
    /// Time resolution (seconds per frame)
    pub time_resolution: f64,
}

/// Compute spectrogram of a 1D signal.
///
/// Returns a time-frequency matrix suitable as CNN input.
pub fn compute_spectrogram(
    signal: &[f64],
    sample_rate: f64,
    config: &SpectrogramConfig,
) -> Result<Spectrogram, SpectrogramError> {
    if signal.len() < config.window_size {
        return Err(SpectrogramError::SignalTooShort {
            signal_len: signal.len(),
            window_size: config.window_size,
        });
    }
    if config.hop_size == 0 {
        return Err(SpectrogramError::InvalidHopSize);
    }
    if config.window_size == 0 {
        return Err(SpectrogramError::InvalidWindowSize);
    }

    let n_frames = (signal.len() - config.window_size) / config.hop_size + 1;
    let n_freq = config.window_size / 2 + 1;
    let window = make_window(config.window_fn, config.window_size);

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(config.window_size);

    let mut data = Array2::zeros((n_freq, n_frames));

    for frame in 0..n_frames {
        let start = frame * config.hop_size;
        let end = start + config.window_size;

        // Apply window and convert to complex
        let mut buffer: Vec<Complex64> = signal[start..end]
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| Complex64::new(s * w, 0.0))
            .collect();

        fft.process(&mut buffer);

        // Store positive frequencies
        for bin in 0..n_freq {
            let mag = buffer[bin].norm();
            data[[bin, frame]] = if config.power { mag * mag } else { mag };
        }
    }

    Ok(Spectrogram {
        data,
        n_freq,
        n_time: n_frames,
        freq_resolution: sample_rate / config.window_size as f64,
        time_resolution: config.hop_size as f64 / sample_rate,
    })
}

/// Compute spectrogram for each subcarrier from a temporal CSI matrix.
///
/// Input: `csi_temporal` is (num_samples × num_subcarriers) amplitude matrix.
/// Returns one spectrogram per subcarrier.
pub fn compute_multi_subcarrier_spectrogram(
    csi_temporal: &Array2<f64>,
    sample_rate: f64,
    config: &SpectrogramConfig,
) -> Result<Vec<Spectrogram>, SpectrogramError> {
    let (_, n_sc) = csi_temporal.dim();
    let mut spectrograms = Vec::with_capacity(n_sc);

    for sc in 0..n_sc {
        let col: Vec<f64> = csi_temporal.column(sc).to_vec();
        spectrograms.push(compute_spectrogram(&col, sample_rate, config)?);
    }

    Ok(spectrograms)
}

/// Generate a window function.
fn make_window(kind: WindowFunction, size: usize) -> Vec<f64> {
    match kind {
        WindowFunction::Rectangular => vec![1.0; size],
        WindowFunction::Hann => (0..size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / (size - 1) as f64).cos()))
            .collect(),
        WindowFunction::Hamming => (0..size)
            .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / (size - 1) as f64).cos())
            .collect(),
        WindowFunction::Blackman => (0..size)
            .map(|i| {
                let n = (size - 1) as f64;
                0.42 - 0.5 * (2.0 * PI * i as f64 / n).cos()
                    + 0.08 * (4.0 * PI * i as f64 / n).cos()
            })
            .collect(),
    }
}

/// Apply attention-gating to a computed CSI spectrogram using ruvector-attn-mincut.
///
/// Treats each time frame as an attention token (d = n_freq_bins features,
/// seq_len = n_time_frames tokens). Self-attention (Q=K=V) gates coherent
/// body-motion frames and suppresses uncorrelated noise/interference frames.
///
/// # Arguments
/// * `spectrogram` - Row-major [n_freq_bins × n_time_frames] f32 slice
/// * `n_freq` - Number of frequency bins (feature dimension d)
/// * `n_time` - Number of time frames (sequence length)
/// * `lambda` - Gating strength: 0.1 = mild, 0.3 = moderate, 0.5 = aggressive
///
/// # Returns
/// Gated spectrogram as Vec<f32>, same shape as input
pub fn gate_spectrogram(
    spectrogram: &[f32],
    n_freq: usize,
    n_time: usize,
    lambda: f32,
) -> Vec<f32> {
    debug_assert_eq!(spectrogram.len(), n_freq * n_time,
        "spectrogram length must equal n_freq * n_time");

    if n_freq == 0 || n_time == 0 {
        return spectrogram.to_vec();
    }

    // Q = K = V = spectrogram (self-attention over time frames)
    let result = attn_mincut(
        spectrogram,
        spectrogram,
        spectrogram,
        n_freq,  // d = feature dimension
        n_time,  // seq_len = time tokens
        lambda,
        /*tau=*/ 2,
        /*eps=*/ 1e-7_f32,
    );
    result.output
}

/// Errors from spectrogram computation.
#[derive(Debug, thiserror::Error)]
pub enum SpectrogramError {
    #[error("Signal too short ({signal_len} samples) for window size {window_size}")]
    SignalTooShort { signal_len: usize, window_size: usize },

    #[error("Hop size must be > 0")]
    InvalidHopSize,

    #[error("Window size must be > 0")]
    InvalidWindowSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrogram_dimensions() {
        let sample_rate = 100.0;
        let signal: Vec<f64> = (0..1000)
            .map(|i| (i as f64 / sample_rate * 2.0 * PI * 5.0).sin())
            .collect();

        let config = SpectrogramConfig {
            window_size: 128,
            hop_size: 32,
            window_fn: WindowFunction::Hann,
            power: true,
        };

        let spec = compute_spectrogram(&signal, sample_rate, &config).unwrap();
        assert_eq!(spec.n_freq, 65); // 128/2 + 1
        assert_eq!(spec.n_time, (1000 - 128) / 32 + 1); // 28 frames
        assert_eq!(spec.data.dim(), (65, 28));
    }

    #[test]
    fn test_single_frequency_peak() {
        // A pure 10 Hz tone at 100 Hz sampling → peak at bin 10/100*256 ≈ bin 26
        let sample_rate = 100.0;
        let freq = 10.0;
        let signal: Vec<f64> = (0..1024)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin())
            .collect();

        let config = SpectrogramConfig {
            window_size: 256,
            hop_size: 128,
            window_fn: WindowFunction::Hann,
            power: true,
        };

        let spec = compute_spectrogram(&signal, sample_rate, &config).unwrap();

        // Find peak frequency bin in the first frame
        let frame = spec.data.column(0);
        let peak_bin = frame
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        let peak_freq = peak_bin as f64 * spec.freq_resolution;
        assert!(
            (peak_freq - freq).abs() < spec.freq_resolution * 2.0,
            "Peak at {:.1} Hz, expected {:.1} Hz",
            peak_freq,
            freq
        );
    }

    #[test]
    fn test_window_functions_symmetric() {
        for wf in [
            WindowFunction::Hann,
            WindowFunction::Hamming,
            WindowFunction::Blackman,
        ] {
            let w = make_window(wf, 64);
            for i in 0..32 {
                assert!(
                    (w[i] - w[63 - i]).abs() < 1e-10,
                    "{:?} not symmetric at {}",
                    wf,
                    i
                );
            }
        }
    }

    #[test]
    fn test_rectangular_window_all_ones() {
        let w = make_window(WindowFunction::Rectangular, 100);
        assert!(w.iter().all(|&v| (v - 1.0).abs() < 1e-10));
    }

    #[test]
    fn test_signal_too_short() {
        let signal = vec![1.0; 10];
        let config = SpectrogramConfig {
            window_size: 256,
            ..Default::default()
        };
        assert!(matches!(
            compute_spectrogram(&signal, 100.0, &config),
            Err(SpectrogramError::SignalTooShort { .. })
        ));
    }

    #[test]
    fn test_multi_subcarrier() {
        let n_samples = 500;
        let n_sc = 8;
        let csi = Array2::from_shape_fn((n_samples, n_sc), |(t, sc)| {
            let freq = 1.0 + sc as f64 * 0.5;
            (2.0 * PI * freq * t as f64 / 100.0).sin()
        });

        let config = SpectrogramConfig {
            window_size: 128,
            hop_size: 64,
            ..Default::default()
        };

        let specs = compute_multi_subcarrier_spectrogram(&csi, 100.0, &config).unwrap();
        assert_eq!(specs.len(), n_sc);
        for spec in &specs {
            assert_eq!(spec.n_freq, 65);
        }
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn gate_spectrogram_preserves_shape() {
        let n_freq = 16_usize;
        let n_time = 10_usize;
        let spectrogram: Vec<f32> = (0..n_freq * n_time).map(|i| i as f32 * 0.01).collect();
        let gated = gate_spectrogram(&spectrogram, n_freq, n_time, 0.3);
        assert_eq!(gated.len(), n_freq * n_time);
    }

    #[test]
    fn gate_spectrogram_zero_lambda_is_identity_ish() {
        let n_freq = 8_usize;
        let n_time = 4_usize;
        let spectrogram: Vec<f32> = vec![1.0; n_freq * n_time];
        // Uniform input — gated output should also be approximately uniform
        let gated = gate_spectrogram(&spectrogram, n_freq, n_time, 0.01);
        assert_eq!(gated.len(), n_freq * n_time);
        // All values should be finite
        assert!(gated.iter().all(|x| x.is_finite()));
    }
}

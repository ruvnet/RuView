//! Attention-mincut spectrogram gating (ruvector-attn-mincut).
//!
//! [`gate_spectrogram`] applies the `attn_mincut` operator to a flat
//! time-frequency spectrogram, suppressing noise frames while amplifying
//! body-motion periods.  The operator treats frequency bins as the feature
//! dimension and time frames as the sequence dimension.

use ruvector_attn_mincut::attn_mincut;

/// Apply attention-mincut gating to a flat spectrogram `[n_freq * n_time]`.
///
/// Suppresses noise frames and amplifies body-motion periods.
///
/// # Arguments
///
/// - `spectrogram`: flat row-major `[n_freq * n_time]` array.
/// - `n_freq`: number of frequency bins (feature dimension `d`).
/// - `n_time`: number of time frames (sequence length).
/// - `lambda`: min-cut threshold — `0.1` = mild gating, `0.5` = aggressive.
///
/// # Returns
///
/// Gated spectrogram of the same length `n_freq * n_time`.
pub fn gate_spectrogram(spectrogram: &[f32], n_freq: usize, n_time: usize, lambda: f32) -> Vec<f32> {
    let out = attn_mincut(
        spectrogram,  // q
        spectrogram,  // k
        spectrogram,  // v
        n_freq,       // d: feature dimension
        n_time,       // seq_len: number of time frames
        lambda,       // lambda: min-cut threshold
        2,            // tau: temporal hysteresis window
        1e-7_f32,     // eps: numerical epsilon
    );
    out.output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_spectrogram_output_length() {
        let n_freq = 4;
        let n_time = 8;
        let spectrogram: Vec<f32> = (0..n_freq * n_time).map(|i| i as f32 * 0.01).collect();
        let gated = gate_spectrogram(&spectrogram, n_freq, n_time, 0.1);
        assert_eq!(
            gated.len(),
            n_freq * n_time,
            "output length must equal n_freq * n_time = {}",
            n_freq * n_time
        );
    }

    #[test]
    fn gate_spectrogram_aggressive_lambda() {
        let n_freq = 4;
        let n_time = 8;
        let spectrogram: Vec<f32> = (0..n_freq * n_time).map(|i| (i as f32).sin()).collect();
        let gated = gate_spectrogram(&spectrogram, n_freq, n_time, 0.5);
        assert_eq!(gated.len(), n_freq * n_time);
    }
}

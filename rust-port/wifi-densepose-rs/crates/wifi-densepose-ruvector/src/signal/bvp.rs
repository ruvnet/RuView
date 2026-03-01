//! Attention-weighted BVP aggregation (ruvector-attention).
//!
//! [`attention_weighted_bvp`] combines per-subcarrier STFT rows using
//! scaled dot-product attention, weighted by per-subcarrier sensitivity
//! scores, to produce a single robust BVP (body velocity profile) vector.

use ruvector_attention::attention::ScaledDotProductAttention;
use ruvector_attention::traits::Attention;

/// Compute attention-weighted BVP aggregation across subcarriers.
///
/// `stft_rows`: one row per subcarrier, each row is `[n_velocity_bins]`.
/// `sensitivity`: per-subcarrier weight.
/// Returns weighted aggregation of length `n_velocity_bins`.
///
/// # Arguments
///
/// - `stft_rows`: one STFT row per subcarrier; each row has `n_velocity_bins`
///   elements representing the Doppler velocity spectrum.
/// - `sensitivity`: per-subcarrier sensitivity weight (same length as
///   `stft_rows`). Higher values cause the corresponding subcarrier to
///   contribute more to the initial query vector.
/// - `n_velocity_bins`: number of Doppler velocity bins in each STFT row.
///
/// # Returns
///
/// Attention-weighted aggregation vector of length `n_velocity_bins`.
/// Returns all-zeros on empty input or zero velocity bins.
pub fn attention_weighted_bvp(
    stft_rows: &[Vec<f32>],
    sensitivity: &[f32],
    n_velocity_bins: usize,
) -> Vec<f32> {
    if stft_rows.is_empty() || n_velocity_bins == 0 {
        return vec![0.0; n_velocity_bins];
    }

    let sens_sum: f32 = sensitivity.iter().sum::<f32>().max(f32::EPSILON);

    // Build the weighted-mean query vector across all subcarriers.
    let query: Vec<f32> = (0..n_velocity_bins)
        .map(|v| {
            stft_rows
                .iter()
                .zip(sensitivity.iter())
                .map(|(row, &s)| row[v] * s)
                .sum::<f32>()
                / sens_sum
        })
        .collect();

    let attn = ScaledDotProductAttention::new(n_velocity_bins);
    let keys: Vec<&[f32]> = stft_rows.iter().map(|r| r.as_slice()).collect();
    let values: Vec<&[f32]> = stft_rows.iter().map(|r| r.as_slice()).collect();

    attn.compute(&query, &keys, &values)
        .unwrap_or_else(|_| vec![0.0; n_velocity_bins])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attention_bvp_output_length() {
        let n_subcarriers = 3;
        let n_velocity_bins = 8;

        let stft_rows: Vec<Vec<f32>> = (0..n_subcarriers)
            .map(|sc| (0..n_velocity_bins).map(|v| (sc * n_velocity_bins + v) as f32 * 0.1).collect())
            .collect();
        let sensitivity = vec![0.5_f32, 0.3, 0.8];

        let result = attention_weighted_bvp(&stft_rows, &sensitivity, n_velocity_bins);
        assert_eq!(
            result.len(),
            n_velocity_bins,
            "output must have length n_velocity_bins = {n_velocity_bins}"
        );
    }

    #[test]
    fn attention_bvp_empty_input_returns_zeros() {
        let result = attention_weighted_bvp(&[], &[], 8);
        assert_eq!(result, vec![0.0_f32; 8]);
    }

    #[test]
    fn attention_bvp_zero_bins_returns_empty() {
        let stft_rows = vec![vec![1.0_f32, 2.0]];
        let sensitivity = vec![1.0_f32];
        let result = attention_weighted_bvp(&stft_rows, &sensitivity, 0);
        assert!(result.is_empty());
    }
}

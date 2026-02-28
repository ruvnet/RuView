//! Normalization operations.
//!
//! Provides LayerNorm and optional RMSNorm implementations.

/// Layer normalization.
///
/// Computes: y = gamma * (x - mean) / sqrt(var + eps) + beta
///
/// # Arguments
///
/// * `input` - Input tensor, shape [n]
/// * `gamma` - Scale parameter, shape [n]
/// * `beta` - Shift parameter, shape [n]
/// * `eps` - Small constant for numerical stability
/// * `output` - Output buffer, shape [n]
#[inline]
pub fn layer_norm(input: &[f32], gamma: &[f32], beta: &[f32], eps: f32, output: &mut [f32]) {
    let n = input.len();
    debug_assert_eq!(gamma.len(), n);
    debug_assert_eq!(beta.len(), n);
    debug_assert_eq!(output.len(), n);

    // Compute mean
    let sum: f32 = input.iter().sum();
    let mean = sum / (n as f32);

    // Compute variance
    let var_sum: f32 = input.iter().map(|&x| (x - mean) * (x - mean)).sum();
    let var = var_sum / (n as f32);

    // Normalize
    let inv_std = 1.0 / (var + eps).sqrt();

    for i in 0..n {
        output[i] = gamma[i] * (input[i] - mean) * inv_std + beta[i];
    }
}

/// In-place layer normalization.
///
/// Modifies input buffer directly.
#[inline]
pub fn layer_norm_inplace(data: &mut [f32], gamma: &[f32], beta: &[f32], eps: f32) {
    let n = data.len();
    debug_assert_eq!(gamma.len(), n);
    debug_assert_eq!(beta.len(), n);

    // Compute mean
    let sum: f32 = data.iter().sum();
    let mean = sum / (n as f32);

    // Compute variance
    let var_sum: f32 = data.iter().map(|&x| (x - mean) * (x - mean)).sum();
    let var = var_sum / (n as f32);

    // Normalize in place
    let inv_std = 1.0 / (var + eps).sqrt();

    for i in 0..n {
        data[i] = gamma[i] * (data[i] - mean) * inv_std + beta[i];
    }
}

/// RMS normalization.
///
/// Computes: y = gamma * x / sqrt(mean(x^2) + eps)
///
/// RMSNorm is faster than LayerNorm as it doesn't compute mean subtraction.
#[inline]
#[cfg(feature = "rmsnorm")]
pub fn rms_norm(input: &[f32], gamma: &[f32], eps: f32, output: &mut [f32]) {
    let n = input.len();
    debug_assert_eq!(gamma.len(), n);
    debug_assert_eq!(output.len(), n);

    // Compute mean of squares
    let sum_sq: f32 = input.iter().map(|&x| x * x).sum();
    let rms = (sum_sq / (n as f32) + eps).sqrt();
    let inv_rms = 1.0 / rms;

    for i in 0..n {
        output[i] = gamma[i] * input[i] * inv_rms;
    }
}

#[cfg(not(feature = "rmsnorm"))]
pub fn rms_norm(input: &[f32], gamma: &[f32], eps: f32, output: &mut [f32]) {
    let n = input.len();
    debug_assert_eq!(gamma.len(), n);
    debug_assert_eq!(output.len(), n);

    let sum_sq: f32 = input.iter().map(|&x| x * x).sum();
    let rms = (sum_sq / (n as f32) + eps).sqrt();
    let inv_rms = 1.0 / rms;

    for i in 0..n {
        output[i] = gamma[i] * input[i] * inv_rms;
    }
}

/// RMS normalization in-place.
#[inline]
pub fn rms_norm_inplace(data: &mut [f32], gamma: &[f32], eps: f32) {
    let n = data.len();
    debug_assert_eq!(gamma.len(), n);

    let sum_sq: f32 = data.iter().map(|&x| x * x).sum();
    let rms = (sum_sq / (n as f32) + eps).sqrt();
    let inv_rms = 1.0 / rms;

    for i in 0..n {
        data[i] = gamma[i] * data[i] * inv_rms;
    }
}

/// Convert int8 to f32 for normalization.
#[inline]
pub fn i8_to_f32(input: &[i8], scale: f32, output: &mut [f32]) {
    debug_assert_eq!(input.len(), output.len());
    for (i, &v) in input.iter().enumerate() {
        output[i] = (v as f32) * scale;
    }
}

/// Convert f32 to int8 after normalization.
#[inline]
pub fn f32_to_i8(input: &[f32], scale: f32, output: &mut [i8]) {
    debug_assert_eq!(input.len(), output.len());
    let inv_scale = 1.0 / scale;
    for (i, &v) in input.iter().enumerate() {
        let q = (v * inv_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_norm() {
        let input = [1.0, 2.0, 3.0, 4.0];
        let gamma = [1.0, 1.0, 1.0, 1.0];
        let beta = [0.0, 0.0, 0.0, 0.0];
        let mut output = [0.0; 4];

        layer_norm(&input, &gamma, &beta, 1e-5, &mut output);

        // Check mean is ~0
        let mean: f32 = output.iter().sum::<f32>() / 4.0;
        assert!(mean.abs() < 1e-5);

        // Check variance is ~1
        let var: f32 = output.iter().map(|&x| x * x).sum::<f32>() / 4.0;
        assert!((var - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_layer_norm_with_params() {
        let input = [0.0, 0.0, 0.0, 0.0];
        let gamma = [2.0, 2.0, 2.0, 2.0];
        let beta = [1.0, 1.0, 1.0, 1.0];
        let mut output = [0.0; 4];

        layer_norm(&input, &gamma, &beta, 1e-5, &mut output);

        // All zeros normalized stay zero, then beta shifts to 1
        for &o in &output {
            assert!((o - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_rms_norm() {
        let input = [1.0, 1.0, 1.0, 1.0];
        let gamma = [1.0, 1.0, 1.0, 1.0];
        let mut output = [0.0; 4];

        rms_norm(&input, &gamma, 1e-5, &mut output);

        // RMS of [1, 1, 1, 1] is 1, so output should be [1, 1, 1, 1]
        for &o in &output {
            assert!((o - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_i8_f32_conversion() {
        let i8_data: [i8; 4] = [127, -128, 0, 64];
        let scale = 0.01;
        let mut f32_data = [0.0; 4];

        i8_to_f32(&i8_data, scale, &mut f32_data);

        assert!((f32_data[0] - 1.27).abs() < 1e-5);
        assert!((f32_data[1] - (-1.28)).abs() < 1e-5);
        assert!((f32_data[2] - 0.0).abs() < 1e-5);
        assert!((f32_data[3] - 0.64).abs() < 1e-5);
    }

    #[test]
    fn test_layer_norm_inplace() {
        let mut data = [1.0, 2.0, 3.0, 4.0];
        let gamma = [1.0, 1.0, 1.0, 1.0];
        let beta = [0.0, 0.0, 0.0, 0.0];

        layer_norm_inplace(&mut data, &gamma, &beta, 1e-5);

        let mean: f32 = data.iter().sum::<f32>() / 4.0;
        assert!(mean.abs() < 1e-5);
    }
}

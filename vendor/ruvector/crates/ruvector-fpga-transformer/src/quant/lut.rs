//! Lookup table implementations for fixed-point operations
//!
//! Provides LUT-based exp, log, and softmax for deterministic computation.

/// LUT-based exponential function
/// Input: Q8.8 fixed point [-16, 16)
/// Output: Q0.16 fixed point [0, 1)
const EXP_LUT_SIZE: usize = 256;
const EXP_LUT_SHIFT: i32 = 8; // Q8.8 input

/// Precomputed exp LUT for range [-8, 8) in Q8.8
static EXP_LUT: [u16; EXP_LUT_SIZE] = generate_exp_lut();

/// Generate exp LUT at compile time
const fn generate_exp_lut() -> [u16; EXP_LUT_SIZE] {
    let mut lut = [0u16; EXP_LUT_SIZE];
    let mut i = 0;
    while i < EXP_LUT_SIZE {
        // Convert index to Q8.8 value (range -128..128 in fixed point = -0.5..0.5)
        let x_q = (i as i32) - 128;
        // Scale to get reasonable exp range
        let x_f = (x_q as f64) / 32.0; // x in [-4, 4)

        // Compute exp and scale to Q0.16
        let exp_val = const_exp(x_f);
        let scaled = exp_val / (1.0 + const_exp(4.0)); // Normalize

        // Convert to u16
        lut[i] = if scaled > 1.0 {
            65535
        } else if scaled < 0.0 {
            0
        } else {
            (scaled * 65535.0) as u16
        };

        i += 1;
    }
    lut
}

/// Const-compatible exp approximation using Taylor series
const fn const_exp(x: f64) -> f64 {
    // exp(x) ≈ 1 + x + x²/2 + x³/6 + x⁴/24 + x⁵/120
    let x2 = x * x;
    let x3 = x2 * x;
    let x4 = x3 * x;
    let x5 = x4 * x;

    1.0 + x + x2 / 2.0 + x3 / 6.0 + x4 / 24.0 + x5 / 120.0
}

/// LUT-based exponential
/// Input: i16 in Q8.8 format
/// Output: u16 in Q0.16 format
#[inline]
pub fn exp_lut(x: i16) -> u16 {
    // Clamp to LUT range
    let clamped = x.clamp(-128 * 256, 127 * 256);
    // Scale to LUT index
    let idx = ((clamped >> EXP_LUT_SHIFT) + 128) as usize;
    EXP_LUT[idx.min(EXP_LUT_SIZE - 1)]
}

/// Log LUT for Q0.16 input
static LOG_LUT: [i16; 256] = generate_log_lut();

const fn generate_log_lut() -> [i16; 256] {
    let mut lut = [0i16; 256];
    let mut i = 1;
    while i < 256 {
        // Input is scaled by 256, so x = i/256 in [0.004, 1)
        let x = (i as f64) / 256.0;
        // log(x) in Q8.8 format
        let log_val = const_ln(x);
        lut[i] = (log_val * 256.0) as i16;
        i += 1;
    }
    lut[0] = i16::MIN; // log(0) = -inf, use min value
    lut
}

/// Const-compatible natural log approximation
const fn const_ln(x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NEG_INFINITY;
    }
    // Use series expansion around x=1: ln(x) = 2 * sum((x-1)/(x+1))^(2n+1)/(2n+1)
    let y = (x - 1.0) / (x + 1.0);
    let y2 = y * y;

    // ln(x) ≈ 2 * (y + y³/3 + y⁵/5 + y⁷/7 + y⁹/9)
    let y3 = y2 * y;
    let y5 = y3 * y2;
    let y7 = y5 * y2;
    let y9 = y7 * y2;

    2.0 * (y + y3 / 3.0 + y5 / 5.0 + y7 / 7.0 + y9 / 9.0)
}

/// LUT-based natural log
/// Input: u16 in Q0.16 format (0 to 65535 = 0.0 to ~1.0)
/// Output: i16 in Q8.8 format
#[inline]
pub fn log_lut(x: u16) -> i16 {
    if x == 0 {
        return i16::MIN;
    }
    // Scale to LUT index
    let idx = (x >> 8) as usize;
    LOG_LUT[idx.min(255)]
}

/// Softmax using LUT-based exp
/// Operates in-place on Q8.8 logits, outputs Q0.16 probabilities
pub fn softmax_lut_q(logits: &mut [i16]) {
    if logits.is_empty() {
        return;
    }

    // Find max for numerical stability
    let max = *logits.iter().max().unwrap_or(&0);

    // Compute exp(x - max) using LUT
    let mut sum: u32 = 0;
    let mut exp_values: Vec<u16> = Vec::with_capacity(logits.len());

    for &logit in logits.iter() {
        let shifted = logit.saturating_sub(max);
        let exp_val = exp_lut(shifted);
        exp_values.push(exp_val);
        sum += exp_val as u32;
    }

    // Normalize
    if sum == 0 {
        sum = 1;
    }

    for (i, logit) in logits.iter_mut().enumerate() {
        let prob = ((exp_values[i] as u64 * 65535) / sum as u64) as u16;
        *logit = prob as i16;
    }
}

/// Softmax on f32 values using LUT (for compatibility)
pub fn softmax_lut(logits: &mut [f32]) {
    if logits.is_empty() {
        return;
    }

    // Find max
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp
    let mut sum = 0.0f32;
    for v in logits.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }

    // Normalize
    if sum > 0.0 {
        for v in logits.iter_mut() {
            *v /= sum;
        }
    }
}

/// Piecewise linear softmax approximation
/// More accurate than LUT but still deterministic
pub fn softmax_pwl(logits: &mut [i16]) {
    if logits.is_empty() {
        return;
    }

    let max = *logits.iter().max().unwrap_or(&0);

    // Piecewise linear exp approximation
    // exp(x) ≈ 1 + x for x near 0
    // exp(x) ≈ 2^(x/ln2) for larger x
    let mut sum: i64 = 0;
    let mut exp_values: Vec<i32> = Vec::with_capacity(logits.len());

    for &logit in logits.iter() {
        let x = (logit - max) as i32; // x <= 0

        // Piecewise approximation (in Q8.8)
        let exp_val = if x >= -256 {
            // x in [-1, 0] -> linear: 1 + x
            (256 + x).max(0) as i32
        } else if x >= -2048 {
            // x in [-8, -1] -> exponential decay
            let shifted = (x + 2048) >> 3; // Scale to 0-256 range
            (shifted * shifted / 256).max(1) as i32
        } else {
            // x < -8 -> essentially zero
            1
        };

        exp_values.push(exp_val);
        sum += exp_val as i64;
    }

    // Normalize to Q0.16
    if sum == 0 {
        sum = 1;
    }

    for (i, logit) in logits.iter_mut().enumerate() {
        let prob = (exp_values[i] as i64 * 65535 / sum) as i16;
        *logit = prob;
    }
}

/// GELU approximation using LUT
/// GELU(x) ≈ 0.5 * x * (1 + tanh(sqrt(2/π) * (x + 0.044715 * x³)))
pub fn gelu_lut(x: i16) -> i16 {
    // Simplified approximation: GELU(x) ≈ x * sigmoid(1.702 * x)
    let scaled = ((x as i32 * 435) >> 8) as i16; // 1.702 * x in Q8.8
    let sigmoid_val = sigmoid_lut(scaled);
    ((x as i32 * sigmoid_val as i32) >> 15) as i16
}

/// Sigmoid LUT
static SIGMOID_LUT: [u16; 256] = generate_sigmoid_lut();

const fn generate_sigmoid_lut() -> [u16; 256] {
    let mut lut = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        // Map index to x in [-8, 8)
        let x = ((i as i32) - 128) as f64 / 16.0;
        // sigmoid(x) = 1 / (1 + exp(-x))
        let sig = 1.0 / (1.0 + const_exp(-x));
        lut[i] = (sig * 65535.0) as u16;
        i += 1;
    }
    lut
}

/// LUT-based sigmoid
/// Input: i16 in Q8.8 format
/// Output: u16 in Q0.16 format
#[inline]
pub fn sigmoid_lut(x: i16) -> u16 {
    // Scale to LUT range
    let idx = (((x >> 5) + 128) as usize).min(255);
    SIGMOID_LUT[idx]
}

/// SiLU (Swish) using sigmoid LUT
/// SiLU(x) = x * sigmoid(x)
#[inline]
pub fn silu_lut(x: i16) -> i16 {
    let sigmoid_val = sigmoid_lut(x);
    ((x as i32 * sigmoid_val as i32) >> 16) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp_lut() {
        // exp(0) should return a non-zero value
        let result = exp_lut(0);
        assert!(result > 0, "exp(0) should be positive");

        // exp is monotonically increasing
        let result_neg = exp_lut(-256); // -1.0 in Q8.8
        let result_zero = exp_lut(0);
        let result_pos = exp_lut(256); // 1.0 in Q8.8
        assert!(
            result_neg <= result_zero,
            "exp should be monotonically increasing"
        );
        assert!(
            result_zero <= result_pos,
            "exp should be monotonically increasing"
        );
    }

    #[test]
    fn test_sigmoid_lut() {
        // sigmoid(0) = 0.5
        let result = sigmoid_lut(0);
        let expected = 32768u16; // 0.5 in Q0.16
        assert!(
            (result as i32 - expected as i32).abs() < 5000,
            "sigmoid(0) ≈ 0.5"
        );

        // sigmoid is monotonically increasing
        let result_neg = sigmoid_lut(-1024);
        let result_zero = sigmoid_lut(0);
        let result_pos = sigmoid_lut(1024);
        assert!(
            result_neg < result_zero,
            "sigmoid should be monotonically increasing"
        );
        assert!(
            result_zero < result_pos,
            "sigmoid should be monotonically increasing"
        );
    }

    #[test]
    fn test_softmax_lut() {
        let mut logits = vec![1.0f32, 2.0, 3.0, 4.0];
        softmax_lut(&mut logits);

        // Sum should be 1.0
        let sum: f32 = logits.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);

        // Should be increasing
        for i in 1..logits.len() {
            assert!(logits[i] > logits[i - 1]);
        }
    }

    #[test]
    fn test_gelu_lut() {
        // GELU(0) should be approximately 0
        assert!(gelu_lut(0).abs() < 100);

        // GELU maintains sign
        let neg_result = gelu_lut(-256);
        assert!(neg_result <= 0, "GELU of negative should be non-positive");

        // GELU of positive values should be positive
        let pos_result = gelu_lut(256);
        assert!(pos_result > 0, "GELU of positive should be positive");
    }
}

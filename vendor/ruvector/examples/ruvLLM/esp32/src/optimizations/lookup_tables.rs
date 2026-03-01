//! Lookup Tables for Fast Fixed-Point Operations
//!
//! Pre-computed tables for softmax, exp, and distance operations.
//! Critical for ESP32 which lacks FPU on most variants.

/// Softmax lookup table (256 entries)
///
/// Pre-computed exp(x) values for x in [-8, 0] range, scaled to INT8.
/// Used for fast fixed-point softmax without floating-point operations.
pub struct SoftmaxLUT {
    /// exp(x) values, scaled by 255
    exp_table: [u8; 256],
    /// Scale factor for input normalization
    input_scale: i32,
}

impl SoftmaxLUT {
    /// Create softmax LUT with default parameters
    pub const fn new() -> Self {
        // Pre-compute exp(x) for x in [-8, 0], scaled to [0, 255]
        // exp(-8) ≈ 0.000335, exp(0) = 1
        // We discretize into 256 bins

        let mut exp_table = [0u8; 256];

        // Approximate exp using polynomial: exp(x) ≈ 1 + x + x²/2 + x³/6
        // For integer approximation: exp(x/32) scaled by 255
        let mut i = 0;
        while i < 256 {
            // x ranges from -8 (i=0) to 0 (i=255)
            // x = (i - 255) / 32
            let x_scaled = i as i32 - 255; // Range: -255 to 0

            // Linear approximation of exp for negative values
            // exp(x) ≈ 255 + x for small |x|, clamped to [1, 255]
            let mut exp_approx = 255 + x_scaled;
            if exp_approx < 1 { exp_approx = 1; }
            if exp_approx > 255 { exp_approx = 255; }
            exp_table[i] = exp_approx as u8;

            i += 1;
        }

        Self {
            exp_table,
            input_scale: 32, // Divide input by 32 before lookup
        }
    }

    /// Look up approximate exp(x) for x in [-8, 0]
    #[inline]
    pub fn exp(&self, x: i32) -> u8 {
        // Clamp x to valid range and scale
        let x_clamped = x.max(-255).min(0);
        let idx = (x_clamped + 255) as usize;
        self.exp_table[idx]
    }

    /// Compute softmax over an array of INT32 logits
    /// Output is scaled by 256 (i.e., 256 = probability 1.0)
    pub fn softmax(&self, logits: &[i32], output: &mut [u16]) {
        if logits.is_empty() {
            return;
        }

        // Find max for numerical stability
        let max_logit = logits.iter().cloned().max().unwrap_or(0);

        // Compute exp and sum
        let mut sum: u32 = 0;
        for (&logit, out) in logits.iter().zip(output.iter_mut()) {
            let x = logit - max_logit;
            let exp_val = self.exp(x) as u16;
            *out = exp_val;
            sum += exp_val as u32;
        }

        // Normalize: probability = exp / sum, scaled by 256
        if sum > 0 {
            for out in output.iter_mut() {
                *out = ((*out as u32 * 256) / sum) as u16;
            }
        }
    }

    /// Fast softmax using only integer operations
    /// Returns probabilities scaled by 256
    pub fn softmax_fast(&self, logits: &mut [i32]) {
        if logits.is_empty() {
            return;
        }

        // Find max
        let max = logits.iter().cloned().max().unwrap_or(0);

        // Subtract max and apply exp approximation
        let mut sum: i32 = 0;
        for logit in logits.iter_mut() {
            let x = (*logit - max).max(-255);
            *logit = self.exp_table[(x + 255) as usize] as i32;
            sum += *logit;
        }

        // Normalize (multiply by 256 then divide by sum)
        if sum > 0 {
            for logit in logits.iter_mut() {
                *logit = (*logit << 8) / sum;
            }
        }
    }
}

impl Default for SoftmaxLUT {
    fn default() -> Self {
        Self::new()
    }
}

/// Exponential lookup table for more precise exp approximation
pub struct ExpLUT {
    /// exp(x/64) for x in [0, 255], scaled by 256
    table: [u16; 256],
}

impl ExpLUT {
    /// Create with higher precision (uses more memory)
    pub const fn new() -> Self {
        let mut table = [0u16; 256];

        let mut i = 0;
        while i < 256 {
            // exp(x/64) for x in [0, 255]
            // At x=0: exp(0) = 1 -> 256
            // At x=255: exp(255/64) ≈ exp(3.98) ≈ 53.5 -> scaled

            // Polynomial approximation: 1 + x + x²/2
            let x = i as i32;
            let x_scaled = x * 256 / 64; // x/64 * 256 for fixed-point
            let x2 = (x_scaled * x_scaled) >> 9; // x² / 512

            let mut exp_val = 256 + x_scaled + (x2 >> 1);
            if exp_val > 65535 { exp_val = 65535; }
            table[i] = exp_val as u16;

            i += 1;
        }

        Self { table }
    }

    /// exp(x) where x is in range [0, 4) scaled by 64
    #[inline]
    pub fn exp(&self, x: u8) -> u16 {
        self.table[x as usize]
    }
}

/// Distance lookup table for common embedding similarities
pub struct DistanceLUT<const SIZE: usize> {
    /// Pre-computed squared differences for INT8 pairs
    sq_diff_table: [u16; 512], // For INT8 diffs in [-255, 255]
}

impl<const SIZE: usize> DistanceLUT<SIZE> {
    /// Create distance LUT
    pub const fn new() -> Self {
        let mut sq_diff_table = [0u16; 512];

        let mut i = 0i32;
        while i < 512 {
            let diff = i - 256; // Map [0, 511] to [-256, 255]
            let mut sq = diff * diff;
            if sq > 65535 { sq = 65535; }
            sq_diff_table[i as usize] = sq as u16;
            i += 1;
        }

        Self { sq_diff_table }
    }

    /// Look up squared difference between two INT8 values
    #[inline]
    pub fn squared_diff(&self, a: i8, b: i8) -> u16 {
        let diff = a as i32 - b as i32;
        let idx = (diff + 256) as usize;
        self.sq_diff_table[idx]
    }

    /// Compute L2 squared distance using lookup table
    pub fn l2_squared(&self, a: &[i8], b: &[i8]) -> u32 {
        debug_assert_eq!(a.len(), b.len());

        let mut sum: u32 = 0;
        for (&x, &y) in a.iter().zip(b.iter()) {
            sum += self.squared_diff(x, y) as u32;
        }
        sum
    }
}

/// Global static lookup tables (no heap allocation)
pub static SOFTMAX_LUT: SoftmaxLUT = SoftmaxLUT::new();
pub static EXP_LUT: ExpLUT = ExpLUT::new();
pub static DISTANCE_LUT: DistanceLUT<256> = DistanceLUT::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_softmax_lut() {
        let lut = SoftmaxLUT::new();

        // exp(0) should be maximum (255)
        assert_eq!(lut.exp(0), 255);

        // exp(-255) should be minimum (1)
        assert_eq!(lut.exp(-255), 1);
    }

    #[test]
    fn test_softmax_normalization() {
        let lut = SoftmaxLUT::new();
        let logits = [100i32, 50, 0, -50];
        let mut output = [0u16; 4];

        lut.softmax(&logits, &mut output);

        // Sum should be approximately 256
        let sum: u16 = output.iter().sum();
        assert!((sum as i32 - 256).abs() < 10);

        // First element should have highest probability
        assert!(output[0] > output[1]);
        assert!(output[1] > output[2]);
        assert!(output[2] > output[3]);
    }

    #[test]
    fn test_distance_lut() {
        let lut = DistanceLUT::<256>::new();

        // Same values: squared diff = 0
        assert_eq!(lut.squared_diff(10, 10), 0);

        // Diff of 10: squared = 100
        assert_eq!(lut.squared_diff(10, 0), 100);
        assert_eq!(lut.squared_diff(0, 10), 100);

        // Negative values
        assert_eq!(lut.squared_diff(-10, 0), 100);
    }

    #[test]
    fn test_l2_distance() {
        let lut = DistanceLUT::<256>::new();

        let a = [10i8, 20, 30, 40];
        let b = [10i8, 20, 30, 40];
        assert_eq!(lut.l2_squared(&a, &b), 0);

        let c = [0i8, 0, 0, 0];
        // (10² + 20² + 30² + 40²) = 100 + 400 + 900 + 1600 = 3000
        assert_eq!(lut.l2_squared(&a, &c), 3000);
    }
}

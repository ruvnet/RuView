//! Lookup Tables for Fast Fixed-Point Operations

/// Softmax lookup table
pub struct SoftmaxLUT {
    exp_table: [u8; 256],
    pub input_scale: i32,
}

impl SoftmaxLUT {
    pub const fn new() -> Self {
        let mut exp_table = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            let x_scaled = i as i32 - 255;
            let mut exp_approx = 255 + x_scaled;
            if exp_approx < 1 { exp_approx = 1; }
            if exp_approx > 255 { exp_approx = 255; }
            exp_table[i] = exp_approx as u8;
            i += 1;
        }
        Self { exp_table, input_scale: 32 }
    }

    #[inline]
    pub fn exp(&self, x: i32) -> u8 {
        let x_clamped = x.max(-255).min(0);
        self.exp_table[(x_clamped + 255) as usize]
    }

    pub fn softmax(&self, logits: &[i32], output: &mut [u16]) {
        if logits.is_empty() { return; }
        let max_logit = logits.iter().cloned().max().unwrap_or(0);
        let mut sum: u32 = 0;
        for (&logit, out) in logits.iter().zip(output.iter_mut()) {
            let exp_val = self.exp(logit - max_logit) as u16;
            *out = exp_val;
            sum += exp_val as u32;
        }
        if sum > 0 {
            for out in output.iter_mut() {
                *out = ((*out as u32 * 256) / sum) as u16;
            }
        }
    }

    pub fn softmax_inplace(&self, logits: &mut [i32]) {
        if logits.is_empty() { return; }
        let max = logits.iter().cloned().max().unwrap_or(0);
        let mut sum: i32 = 0;
        for logit in logits.iter_mut() {
            let x = (*logit - max).max(-255);
            *logit = self.exp_table[(x + 255) as usize] as i32;
            sum += *logit;
        }
        if sum > 0 {
            for logit in logits.iter_mut() {
                *logit = (*logit << 8) / sum;
            }
        }
    }
}

impl Default for SoftmaxLUT {
    fn default() -> Self { Self::new() }
}

/// Exponential lookup table
pub struct ExpLUT {
    table: [u16; 256],
}

impl ExpLUT {
    pub const fn new() -> Self {
        let mut table = [0u16; 256];
        let mut i = 0;
        while i < 256 {
            let x = i as i32;
            let x_scaled = x * 256 / 64;
            let x2 = (x_scaled * x_scaled) >> 9;
            let mut exp_val = 256 + x_scaled + (x2 >> 1);
            if exp_val > 65535 { exp_val = 65535; }
            table[i] = exp_val as u16;
            i += 1;
        }
        Self { table }
    }

    #[inline]
    pub fn exp(&self, x: u8) -> u16 { self.table[x as usize] }
}

/// Distance lookup table for L2 distance
pub struct DistanceLUT<const SIZE: usize> {
    sq_diff_table: [u16; 512],
}

impl<const SIZE: usize> DistanceLUT<SIZE> {
    pub const fn new() -> Self {
        let mut sq_diff_table = [0u16; 512];
        let mut i = 0i32;
        while i < 512 {
            let diff = i - 256;
            let mut sq = diff * diff;
            if sq > 65535 { sq = 65535; }
            sq_diff_table[i as usize] = sq as u16;
            i += 1;
        }
        Self { sq_diff_table }
    }

    #[inline]
    pub fn squared_diff(&self, a: i8, b: i8) -> u16 {
        let idx = (a as i32 - b as i32 + 256) as usize;
        self.sq_diff_table[idx]
    }

    pub fn l2_squared(&self, a: &[i8], b: &[i8]) -> u32 {
        a.iter().zip(b.iter()).map(|(&x, &y)| self.squared_diff(x, y) as u32).sum()
    }
}

pub static SOFTMAX_LUT: SoftmaxLUT = SoftmaxLUT::new();
pub static EXP_LUT: ExpLUT = ExpLUT::new();
pub static DISTANCE_LUT: DistanceLUT<256> = DistanceLUT::new();

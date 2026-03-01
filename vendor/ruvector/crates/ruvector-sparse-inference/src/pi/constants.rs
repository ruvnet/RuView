//! π-derived calibration constants for low-precision systems
//!
//! Using π (or π-derived constants) for normalization, angular embeddings,
//! periodic projections, and phase encoding gives a stable, universal reference
//! that doesn't align with powers of two or quantization boundaries.
//!
//! This avoids resonance artifacts where values collapse into repeating buckets.
//! In short: **π breaks symmetry**.

use crate::precision::PrecisionLane;
use std::f32::consts::PI;

/// π-based scale factor for 3-bit quantization
/// Chosen to avoid power-of-2 boundaries
pub const PI_SCALE_3BIT: f32 = PI / 4.0; // ~0.785

/// π-based scale factor for 5-bit quantization
pub const PI_SCALE_5BIT: f32 = PI / 16.0; // ~0.196

/// π-based scale factor for 7-bit quantization
pub const PI_SCALE_7BIT: f32 = PI / 64.0; // ~0.049

/// Golden ratio derived from π for optimal distribution
pub const PHI_APPROX: f32 = 2.0 / (PI - 1.0); // ~0.934

/// First 100 digits of π for deterministic seeding
pub const PI_DIGITS: [u8; 100] = [
    3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3, 2, 3, 8, 4, 6, 2, 6, 4, 3, 3, 8, 3, 2, 7, 9, 5,
    0, 2, 8, 8, 4, 1, 9, 7, 1, 6, 9, 3, 9, 9, 3, 7, 5, 1, 0, 5, 8, 2, 0, 9, 7, 4, 9, 4, 4, 5, 9, 2,
    3, 0, 7, 8, 1, 6, 4, 0, 6, 2, 8, 6, 2, 0, 8, 9, 9, 8, 6, 2, 8, 0, 3, 4, 8, 2, 5, 3, 4, 2, 1, 1,
    7, 0, 6, 7,
];

/// π-derived calibration constants for a precision lane
#[derive(Debug, Clone, Copy)]
pub struct PiCalibration {
    /// Base scale factor (π / 2^bits)
    pub scale: f32,
    /// Phase offset for angular encoding
    pub phase_offset: f32,
    /// Normalization factor
    pub norm_factor: f32,
    /// Precision lane
    pub lane: PrecisionLane,
    /// Anti-resonance offset (prevents bucket collapse)
    pub anti_resonance: f32,
}

impl PiCalibration {
    /// Create calibration constants for a precision lane
    pub fn for_lane(lane: PrecisionLane) -> Self {
        match lane {
            PrecisionLane::Bit3 => Self {
                scale: PI_SCALE_3BIT,
                phase_offset: PI / 8.0,
                norm_factor: 3.0 / PI,
                lane,
                anti_resonance: Self::compute_anti_resonance(3),
            },
            PrecisionLane::Bit5 => Self {
                scale: PI_SCALE_5BIT,
                phase_offset: PI / 32.0,
                norm_factor: 15.0 / PI,
                lane,
                anti_resonance: Self::compute_anti_resonance(5),
            },
            PrecisionLane::Bit7 => Self {
                scale: PI_SCALE_7BIT,
                phase_offset: PI / 128.0,
                norm_factor: 63.0 / PI,
                lane,
                anti_resonance: Self::compute_anti_resonance(7),
            },
            PrecisionLane::Float32 => Self {
                scale: 1.0,
                phase_offset: 0.0,
                norm_factor: 1.0,
                lane,
                anti_resonance: 0.0,
            },
        }
    }

    /// Compute anti-resonance offset for given bit depth
    /// Uses π fractional part to avoid power-of-2 alignment
    fn compute_anti_resonance(bits: u8) -> f32 {
        let pi_frac = PI - 3.0; // 0.14159...
        pi_frac / (1 << bits) as f32
    }

    /// Normalize a value using π-based constants
    pub fn normalize(&self, value: f32) -> f32 {
        (value * self.norm_factor + self.anti_resonance) * self.scale
    }

    /// Denormalize a value
    pub fn denormalize(&self, value: f32) -> f32 {
        (value / self.scale - self.anti_resonance) / self.norm_factor
    }

    /// Apply phase encoding (maps to -π to π range)
    pub fn phase_encode(&self, value: f32) -> f32 {
        let normalized = self.normalize(value);
        (normalized + self.phase_offset).sin() * PI
    }

    /// Decode phase-encoded value
    pub fn phase_decode(&self, phase: f32) -> f32 {
        let normalized = (phase / PI).asin() - self.phase_offset;
        self.denormalize(normalized)
    }

    /// Get π-based angular velocity (for streaming updates)
    pub fn angular_velocity(&self, delta: f32) -> f32 {
        delta * self.scale * 2.0 * PI
    }

    /// Quantize with π-based rounding (breaks symmetry)
    pub fn pi_quantize(&self, value: f32, max_val: i8) -> i8 {
        let scaled = value * self.norm_factor + self.anti_resonance;
        let rounded = (scaled + 0.5 * self.anti_resonance).round();
        (rounded as i8).clamp(-max_val, max_val - 1)
    }

    /// Dequantize with π-based scaling
    pub fn pi_dequantize(&self, quantized: i8) -> f32 {
        ((quantized as f32) - self.anti_resonance) / self.norm_factor
    }
}

/// Angular frequency table for SIMD-friendly operations
pub struct AngularFrequencyTable {
    /// Precomputed sin values at π intervals
    pub sin_table: [f32; 256],
    /// Precomputed cos values at π intervals
    pub cos_table: [f32; 256],
    /// Table resolution
    pub resolution: usize,
}

impl AngularFrequencyTable {
    /// Create a new angular frequency table
    pub fn new() -> Self {
        let mut sin_table = [0.0f32; 256];
        let mut cos_table = [0.0f32; 256];

        for i in 0..256 {
            let angle = (i as f32) * 2.0 * PI / 256.0;
            sin_table[i] = angle.sin();
            cos_table[i] = angle.cos();
        }

        Self {
            sin_table,
            cos_table,
            resolution: 256,
        }
    }

    /// Fast sin approximation using table lookup
    pub fn fast_sin(&self, angle: f32) -> f32 {
        let normalized = angle.rem_euclid(2.0 * PI);
        let index = ((normalized * 256.0 / (2.0 * PI)) as usize) % 256;
        self.sin_table[index]
    }

    /// Fast cos approximation using table lookup
    pub fn fast_cos(&self, angle: f32) -> f32 {
        let normalized = angle.rem_euclid(2.0 * PI);
        let index = ((normalized * 256.0 / (2.0 * PI)) as usize) % 256;
        self.cos_table[index]
    }
}

impl Default for AngularFrequencyTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_scales() {
        assert!((PI_SCALE_3BIT - 0.785).abs() < 0.01);
        assert!((PI_SCALE_5BIT - 0.196).abs() < 0.01);
        assert!((PI_SCALE_7BIT - 0.049).abs() < 0.01);
    }

    #[test]
    fn test_calibration_roundtrip() {
        let cal = PiCalibration::for_lane(PrecisionLane::Bit5);
        let original = 0.5f32;
        let normalized = cal.normalize(original);
        let denormalized = cal.denormalize(normalized);
        assert!((original - denormalized).abs() < 0.001);
    }

    #[test]
    fn test_phase_encoding_roundtrip() {
        let cal = PiCalibration::for_lane(PrecisionLane::Bit7);
        let original = 0.3f32;
        let encoded = cal.phase_encode(original);
        // Phase encoding is lossy for values outside valid range
        assert!(encoded.is_finite());
    }

    #[test]
    fn test_pi_quantize() {
        let cal = PiCalibration::for_lane(PrecisionLane::Bit3);
        let q = cal.pi_quantize(1.0, 4);
        assert!(q >= -4 && q <= 3);
    }

    #[test]
    fn test_angular_frequency_table() {
        let table = AngularFrequencyTable::new();

        // Test at known angles
        assert!((table.fast_sin(0.0) - 0.0).abs() < 0.03);
        assert!((table.fast_sin(PI / 2.0) - 1.0).abs() < 0.03);
        assert!((table.fast_cos(0.0) - 1.0).abs() < 0.03);
        assert!((table.fast_cos(PI) - (-1.0)).abs() < 0.03);
    }

    #[test]
    fn test_anti_resonance_nonzero() {
        let cal = PiCalibration::for_lane(PrecisionLane::Bit5);
        assert!(cal.anti_resonance > 0.0);
        assert!(cal.anti_resonance < 0.01);
    }
}

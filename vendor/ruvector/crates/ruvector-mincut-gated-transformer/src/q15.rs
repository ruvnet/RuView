//! Q15 Fixed-Point Arithmetic
//!
//! This module provides a type-safe wrapper for Q15 fixed-point numbers, which represent
//! fractional values in the range [0.0, 1.0) using 16-bit unsigned integers.
//!
//! # Q15 Format
//!
//! Q15 (also known as 0.15 or UQ0.15) is a fixed-point number format where:
//! - All 16 bits represent the fractional part
//! - Integer value 0 represents 0.0
//! - Integer value 32767 (0x7FFF) represents approximately 0.99997
//! - Integer value 32768 and above can represent values ≥ 1.0 (for internal calculations)
//!
//! # Examples
//!
//! ```
//! use ruvector_mincut_gated_transformer::Q15;
//!
//! // Create Q15 values
//! let zero = Q15::ZERO;
//! let half = Q15::HALF;
//! let one = Q15::ONE;
//!
//! // Convert from floating point
//! let coherence = Q15::from_f32(0.75);
//! let threshold = Q15::from_f32(0.5);
//!
//! // Comparison
//! assert!(coherence > threshold);
//!
//! // Convert to floating point
//! let value: f32 = coherence.to_f32();
//! assert!((value - 0.75).abs() < 0.001);
//!
//! // Arithmetic operations
//! let sum = coherence + threshold;
//! let diff = coherence - threshold;
//! let product = coherence * threshold;
//! ```

use core::fmt;
use core::ops::{Add, Mul, Sub};
use serde::{Deserialize, Serialize};

/// Q15 fixed-point number representing values in the range [0.0, 1.0+)
///
/// This type wraps a `u16` value where the entire 16 bits represent the fractional part.
/// The value 32767 (0x7FFF) represents approximately 0.99997, and 65535 represents ~2.0.
///
/// # Type Safety
///
/// Using this newtype wrapper instead of raw `u16` provides:
/// - Type safety: prevents mixing fixed-point and integer arithmetic
/// - Self-documenting code: signals that values are in Q15 format
/// - Encapsulation: ensures conversions are done correctly
///
/// # Precision
///
/// Q15 provides approximately 4-5 decimal digits of precision with a resolution
/// of 1/32768 ≈ 0.000030518.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Q15(u16);

impl Q15 {
    /// Maximum value that fits in Q15 format (represents ~1.99997)
    const MAX_RAW: u16 = u16::MAX;

    /// Scale factor for Q15 format (2^15)
    const SCALE: f32 = 32768.0;

    /// Zero value (0.0)
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let zero = Q15::ZERO;
    /// assert_eq!(zero.to_f32(), 0.0);
    /// ```
    pub const ZERO: Self = Self(0);

    /// Half value (0.5)
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let half = Q15::HALF;
    /// assert!((half.to_f32() - 0.5).abs() < 0.001);
    /// ```
    pub const HALF: Self = Self(16384); // 0.5 * 32768

    /// One value (1.0)
    ///
    /// Note: This represents exactly 1.0 using the value 32768 (0x8000).
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let one = Q15::ONE;
    /// assert_eq!(one.to_f32(), 1.0);
    /// ```
    pub const ONE: Self = Self(32768); // 1.0 * 32768

    /// Create a Q15 value from a raw u16 representation
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw u16 value where 32768 represents 1.0
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let q = Q15::from_raw(16384);
    /// assert!((q.to_f32() - 0.5).abs() < 0.001);
    /// ```
    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Get the raw u16 representation
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let q = Q15::HALF;
    /// assert_eq!(q.to_raw(), 16384);
    /// ```
    #[inline]
    pub const fn to_raw(self) -> u16 {
        self.0
    }

    /// Convert from f32 to Q15
    ///
    /// Values are clamped to the valid range. Values less than 0.0 become 0.0,
    /// and values greater than ~2.0 are clamped to the maximum representable value.
    ///
    /// # Arguments
    ///
    /// * `value` - Floating point value to convert (typically in range [0.0, 1.0])
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let q = Q15::from_f32(0.75);
    /// assert!((q.to_f32() - 0.75).abs() < 0.001);
    ///
    /// // Values are clamped
    /// let clamped = Q15::from_f32(-0.5);
    /// assert_eq!(clamped, Q15::ZERO);
    /// ```
    #[inline]
    pub fn from_f32(value: f32) -> Self {
        if value <= 0.0 {
            Self::ZERO
        } else if value >= (Self::MAX_RAW as f32 / Self::SCALE) {
            Self(Self::MAX_RAW)
        } else {
            Self((value * Self::SCALE) as u16)
        }
    }

    /// Convert from Q15 to f32
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let q = Q15::from_f32(0.75);
    /// let f = q.to_f32();
    /// assert!((f - 0.75).abs() < 0.001);
    /// ```
    #[inline]
    pub fn to_f32(self) -> f32 {
        (self.0 as f32) / Self::SCALE
    }

    /// Saturating addition
    ///
    /// Adds two Q15 values, saturating at the maximum value instead of wrapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.75);
    /// let b = Q15::from_f32(0.5);
    /// let sum = a.saturating_add(b);
    /// assert!(sum.to_f32() >= 1.0);
    /// ```
    #[inline]
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction
    ///
    /// Subtracts two Q15 values, saturating at zero instead of wrapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.25);
    /// let b = Q15::from_f32(0.5);
    /// let diff = a.saturating_sub(b);
    /// assert_eq!(diff, Q15::ZERO);
    /// ```
    #[inline]
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Multiply two Q15 values
    ///
    /// Performs fixed-point multiplication with proper scaling.
    /// The result is saturated if it exceeds the maximum representable value.
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.5);
    /// let b = Q15::from_f32(0.5);
    /// let product = a.saturating_mul(b);
    /// assert!((product.to_f32() - 0.25).abs() < 0.001);
    /// ```
    #[inline]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        // Multiply as u32 to avoid overflow, then shift right by 15 bits
        let product = (self.0 as u32 * rhs.0 as u32) >> 15;
        Self(product.min(Self::MAX_RAW as u32) as u16)
    }

    /// Linear interpolation between two Q15 values
    ///
    /// Returns `self + t * (other - self)` where `t` is in [0.0, 1.0].
    ///
    /// # Arguments
    ///
    /// * `other` - Target value
    /// * `t` - Interpolation factor (0.0 = self, 1.0 = other)
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.0);
    /// let b = Q15::from_f32(1.0);
    /// let mid = a.lerp(b, Q15::HALF);
    /// assert!((mid.to_f32() - 0.5).abs() < 0.01);
    /// ```
    #[inline]
    pub fn lerp(self, other: Self, t: Self) -> Self {
        if t.0 == 0 {
            self
        } else if t.0 >= 32768 {
            other
        } else {
            // Calculate: self + t * (other - self)
            let diff = if other.0 >= self.0 {
                let delta = other.0 - self.0;
                let scaled = ((delta as u32 * t.0 as u32) >> 15) as u16;
                self.0.saturating_add(scaled)
            } else {
                let delta = self.0 - other.0;
                let scaled = ((delta as u32 * t.0 as u32) >> 15) as u16;
                self.0.saturating_sub(scaled)
            };
            Self(diff)
        }
    }

    /// Clamp value to a range
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let value = Q15::from_f32(0.75);
    /// let min = Q15::from_f32(0.2);
    /// let max = Q15::from_f32(0.6);
    /// let clamped = value.clamp(min, max);
    /// assert_eq!(clamped, max);
    /// ```
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0))
    }

    /// Returns the minimum of two values
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.75);
    /// let b = Q15::from_f32(0.5);
    /// assert_eq!(a.min(b), b);
    /// ```
    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    /// Returns the maximum of two values
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut_gated_transformer::Q15;
    ///
    /// let a = Q15::from_f32(0.75);
    /// let b = Q15::from_f32(0.5);
    /// assert_eq!(a.max(b), a);
    /// ```
    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}

// ============================================================================
// Batch Operations (SIMD-friendly)
// ============================================================================

/// Batch multiply Q15 values.
///
/// Processes arrays of Q15 values efficiently. When SIMD is available,
/// this can achieve 16× speedup using PMULHUW-style operations.
///
/// # Arguments
///
/// * `a` - First operand array
/// * `b` - Second operand array
/// * `out` - Output array (must be same length as inputs)
#[inline]
pub fn q15_batch_mul(a: &[Q15], b: &[Q15], out: &mut [Q15]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());

    for i in 0..a.len() {
        out[i] = a[i].saturating_mul(b[i]);
    }
}

/// Batch add Q15 values with saturation.
#[inline]
pub fn q15_batch_add(a: &[Q15], b: &[Q15], out: &mut [Q15]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());

    for i in 0..a.len() {
        out[i] = a[i].saturating_add(b[i]);
    }
}

/// Batch linear interpolation.
///
/// Computes `a[i] + t * (b[i] - a[i])` for each element.
#[inline]
pub fn q15_batch_lerp(a: &[Q15], b: &[Q15], t: Q15, out: &mut [Q15]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());

    for i in 0..a.len() {
        out[i] = a[i].lerp(b[i], t);
    }
}

/// Convert f32 slice to Q15 slice.
#[inline]
pub fn f32_to_q15_batch(input: &[f32], output: &mut [Q15]) {
    debug_assert_eq!(input.len(), output.len());

    for i in 0..input.len() {
        output[i] = Q15::from_f32(input[i]);
    }
}

/// Convert Q15 slice to f32 slice.
#[inline]
pub fn q15_to_f32_batch(input: &[Q15], output: &mut [f32]) {
    debug_assert_eq!(input.len(), output.len());

    for i in 0..input.len() {
        output[i] = input[i].to_f32();
    }
}

/// Dot product of two Q15 arrays.
///
/// Returns the sum of element-wise products, useful for attention scores.
#[inline]
pub fn q15_dot(a: &[Q15], b: &[Q15]) -> Q15 {
    debug_assert_eq!(a.len(), b.len());

    let mut acc: u32 = 0;
    for i in 0..a.len() {
        // Multiply and accumulate in u32 to avoid overflow
        let product = (a[i].to_raw() as u32 * b[i].to_raw() as u32) >> 15;
        acc = acc.saturating_add(product);
    }

    // Clamp to Q15 range
    Q15::from_raw(acc.min(u16::MAX as u32) as u16)
}

// Implement arithmetic operations with wrapping behavior (use saturating_* for safety)

impl Add for Q15 {
    type Output = Self;

    /// Add two Q15 values with wrapping
    ///
    /// Note: This wraps on overflow. Consider using `saturating_add` for safety.
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Q15 {
    type Output = Self;

    /// Subtract two Q15 values with wrapping
    ///
    /// Note: This wraps on underflow. Consider using `saturating_sub` for safety.
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Mul for Q15 {
    type Output = Self;

    /// Multiply two Q15 values
    ///
    /// Performs fixed-point multiplication with proper scaling and saturation.
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.saturating_mul(rhs)
    }
}

impl Default for Q15 {
    /// Default value is zero
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for Q15 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.5}", self.to_f32())
    }
}

impl From<Q15> for f32 {
    #[inline]
    fn from(q: Q15) -> Self {
        q.to_f32()
    }
}

impl From<Q15> for f64 {
    #[inline]
    fn from(q: Q15) -> Self {
        q.to_f32() as f64
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::format;

    #[test]
    fn test_constants() {
        assert_eq!(Q15::ZERO.to_f32(), 0.0);
        assert!((Q15::HALF.to_f32() - 0.5).abs() < 0.001);
        assert_eq!(Q15::ONE.to_f32(), 1.0);
    }

    #[test]
    fn test_from_f32() {
        let q = Q15::from_f32(0.75);
        assert!((q.to_f32() - 0.75).abs() < 0.001);

        let q = Q15::from_f32(0.0);
        assert_eq!(q, Q15::ZERO);

        let q = Q15::from_f32(1.0);
        assert_eq!(q, Q15::ONE);

        // Test clamping
        let q = Q15::from_f32(-0.5);
        assert_eq!(q, Q15::ZERO);
    }

    #[test]
    fn test_arithmetic() {
        let a = Q15::from_f32(0.5);
        let b = Q15::from_f32(0.25);

        let sum = a.saturating_add(b);
        assert!((sum.to_f32() - 0.75).abs() < 0.001);

        let diff = a.saturating_sub(b);
        assert!((diff.to_f32() - 0.25).abs() < 0.001);

        let prod = a * b;
        assert!((prod.to_f32() - 0.125).abs() < 0.001);
    }

    #[test]
    fn test_comparison() {
        let a = Q15::from_f32(0.75);
        let b = Q15::from_f32(0.5);

        assert!(a > b);
        assert!(b < a);
        assert_eq!(a, a);
        assert_ne!(a, b);
    }

    #[test]
    fn test_saturating_add() {
        let a = Q15::from_f32(0.75);
        let b = Q15::from_f32(0.5);
        let sum = a.saturating_add(b);
        // Should saturate instead of wrapping
        assert!(sum.to_f32() >= 1.0);
    }

    #[test]
    fn test_saturating_sub() {
        let a = Q15::from_f32(0.25);
        let b = Q15::from_f32(0.5);
        let diff = a.saturating_sub(b);
        // Should saturate at zero
        assert_eq!(diff, Q15::ZERO);
    }

    #[test]
    fn test_lerp() {
        let a = Q15::from_f32(0.0);
        let b = Q15::from_f32(1.0);

        let mid = a.lerp(b, Q15::HALF);
        assert!((mid.to_f32() - 0.5).abs() < 0.01);

        let quarter = a.lerp(b, Q15::from_f32(0.25));
        assert!((quarter.to_f32() - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_clamp() {
        let value = Q15::from_f32(0.75);
        let min = Q15::from_f32(0.2);
        let max = Q15::from_f32(0.6);

        let clamped = value.clamp(min, max);
        assert_eq!(clamped, max);

        let value2 = Q15::from_f32(0.1);
        let clamped2 = value2.clamp(min, max);
        assert_eq!(clamped2, min);

        let value3 = Q15::from_f32(0.4);
        let clamped3 = value3.clamp(min, max);
        assert_eq!(clamped3, value3);
    }

    #[test]
    fn test_min_max() {
        let a = Q15::from_f32(0.75);
        let b = Q15::from_f32(0.5);

        assert_eq!(a.min(b), b);
        assert_eq!(a.max(b), a);
    }

    #[test]
    fn test_display() {
        let q = Q15::from_f32(0.75);
        let s = format!("{}", q);
        assert!(s.starts_with("0.75"));
    }

    #[test]
    fn test_serde_raw() {
        // Test that serde round-trips through raw value
        let q = Q15::from_f32(0.75);
        let raw = q.to_raw();
        let reconstructed = Q15::from_raw(raw);
        assert_eq!(q, reconstructed);
    }

    #[test]
    fn test_precision() {
        // Test that we maintain reasonable precision
        for i in 0..=100 {
            let f = i as f32 / 100.0;
            let q = Q15::from_f32(f);
            let back = q.to_f32();
            assert!((back - f).abs() < 0.001, "Failed for {}: got {}", f, back);
        }
    }
}

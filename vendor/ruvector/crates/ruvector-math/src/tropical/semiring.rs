//! Tropical Semiring Core Operations
//!
//! Implements the max-plus and min-plus semirings.

use std::cmp::Ordering;
use std::ops::{Add, Mul};

/// Tropical number in the max-plus semiring
#[derive(Debug, Clone, Copy)]
pub struct Tropical {
    value: f64,
}

impl Tropical {
    /// Tropical zero (-∞ in max-plus)
    pub const ZERO: Tropical = Tropical {
        value: f64::NEG_INFINITY,
    };

    /// Tropical one (0 in max-plus)
    pub const ONE: Tropical = Tropical { value: 0.0 };

    /// Create new tropical number
    #[inline]
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Get underlying value
    #[inline]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Check if this is tropical zero (-∞)
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.value == f64::NEG_INFINITY
    }

    /// Tropical addition: max(a, b)
    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            value: self.value.max(other.value),
        }
    }

    /// Tropical multiplication: a + b
    #[inline]
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            Self::ZERO
        } else {
            Self {
                value: self.value + other.value,
            }
        }
    }

    /// Tropical power: n * a
    #[inline]
    pub fn pow(&self, n: i32) -> Self {
        if self.is_zero() {
            Self::ZERO
        } else {
            Self {
                value: self.value * n as f64,
            }
        }
    }
}

impl Add for Tropical {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Tropical::add(&self, &other)
    }
}

impl Mul for Tropical {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Tropical::mul(&self, &other)
    }
}

impl PartialEq for Tropical {
    fn eq(&self, other: &Self) -> bool {
        (self.value - other.value).abs() < 1e-10
    }
}

impl PartialOrd for Tropical {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

/// Trait for tropical semiring operations
pub trait TropicalSemiring {
    /// Tropical zero element
    fn tropical_zero() -> Self;

    /// Tropical one element
    fn tropical_one() -> Self;

    /// Tropical addition (max for max-plus, min for min-plus)
    fn tropical_add(&self, other: &Self) -> Self;

    /// Tropical multiplication (ordinary addition)
    fn tropical_mul(&self, other: &Self) -> Self;
}

impl TropicalSemiring for f64 {
    fn tropical_zero() -> Self {
        f64::NEG_INFINITY
    }

    fn tropical_one() -> Self {
        0.0
    }

    fn tropical_add(&self, other: &Self) -> Self {
        self.max(*other)
    }

    fn tropical_mul(&self, other: &Self) -> Self {
        if *self == f64::NEG_INFINITY || *other == f64::NEG_INFINITY {
            f64::NEG_INFINITY
        } else {
            *self + *other
        }
    }
}

/// Min-plus tropical number (for shortest paths)
#[derive(Debug, Clone, Copy)]
pub struct TropicalMin {
    value: f64,
}

impl TropicalMin {
    /// Tropical zero (+∞ in min-plus)
    pub const ZERO: TropicalMin = TropicalMin {
        value: f64::INFINITY,
    };

    /// Tropical one (0 in min-plus)
    pub const ONE: TropicalMin = TropicalMin { value: 0.0 };

    /// Create new min-plus tropical number
    #[inline]
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Get underlying value
    #[inline]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Tropical addition: min(a, b)
    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            value: self.value.min(other.value),
        }
    }

    /// Tropical multiplication: a + b
    #[inline]
    pub fn mul(&self, other: &Self) -> Self {
        if self.value == f64::INFINITY || other.value == f64::INFINITY {
            Self::ZERO
        } else {
            Self {
                value: self.value + other.value,
            }
        }
    }
}

impl Add for TropicalMin {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        TropicalMin::add(&self, &other)
    }
}

impl Mul for TropicalMin {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        TropicalMin::mul(&self, &other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tropical_zero_one() {
        let zero = Tropical::ZERO;
        let one = Tropical::ONE;
        let a = Tropical::new(5.0);

        // Zero is identity for max (use + operator which uses Add trait)
        assert_eq!(zero + a, a);

        // One is identity for + (use * operator which uses Mul trait)
        assert_eq!(one * a, a);
    }

    #[test]
    fn test_tropical_associativity() {
        let a = Tropical::new(1.0);
        let b = Tropical::new(2.0);
        let c = Tropical::new(3.0);

        // (a ⊕ b) ⊕ c = a ⊕ (b ⊕ c)
        assert_eq!((a + b) + c, a + (b + c));

        // (a ⊗ b) ⊗ c = a ⊗ (b ⊗ c)
        assert_eq!((a * b) * c, a * (b * c));
    }

    #[test]
    fn test_tropical_min_plus() {
        let a = TropicalMin::new(3.0);
        let b = TropicalMin::new(5.0);

        assert_eq!((a + b).value(), 3.0); // min(3, 5) = 3
        assert_eq!((a * b).value(), 8.0); // 3 + 5 = 8
    }
}

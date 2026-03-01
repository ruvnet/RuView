//! Filter operator types for metadata-filtered queries and deletes.

/// Filter operator discriminator.
///
/// Comparison operators use the low nibble (0x00..0x07), logical combinators
/// use the 0x10 range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum FilterOp {
    /// field == value
    Eq = 0x00,
    /// field != value
    Ne = 0x01,
    /// field < value
    Lt = 0x02,
    /// field <= value
    Le = 0x03,
    /// field > value
    Gt = 0x04,
    /// field >= value
    Ge = 0x05,
    /// field in [values]
    In = 0x06,
    /// field in [low, high)
    Range = 0x07,
    /// All children must match.
    And = 0x10,
    /// Any child must match.
    Or = 0x11,
    /// Negate single child.
    Not = 0x12,
}

impl FilterOp {
    /// Returns true if this is a logical combinator (AND, OR, NOT).
    #[inline]
    pub const fn is_logical(self) -> bool {
        (self as u8) >= 0x10
    }

    /// Returns true if this is a comparison operator.
    #[inline]
    pub const fn is_comparison(self) -> bool {
        (self as u8) < 0x10
    }
}

impl TryFrom<u8> for FilterOp {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Eq),
            0x01 => Ok(Self::Ne),
            0x02 => Ok(Self::Lt),
            0x03 => Ok(Self::Le),
            0x04 => Ok(Self::Gt),
            0x05 => Ok(Self::Ge),
            0x06 => Ok(Self::In),
            0x07 => Ok(Self::Range),
            0x10 => Ok(Self::And),
            0x11 => Ok(Self::Or),
            0x12 => Ok(Self::Not),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_comparison_ops() {
        for raw in 0x00..=0x07u8 {
            let op = FilterOp::try_from(raw).unwrap();
            assert_eq!(op as u8, raw);
            assert!(op.is_comparison());
            assert!(!op.is_logical());
        }
    }

    #[test]
    fn round_trip_logical_ops() {
        for raw in [0x10u8, 0x11, 0x12] {
            let op = FilterOp::try_from(raw).unwrap();
            assert_eq!(op as u8, raw);
            assert!(op.is_logical());
            assert!(!op.is_comparison());
        }
    }

    #[test]
    fn gap_values_are_invalid() {
        for raw in 0x08..=0x0Fu8 {
            assert_eq!(FilterOp::try_from(raw), Err(raw));
        }
    }
}

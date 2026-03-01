//! Quantization type discriminator for QUANT_SEG payloads.

/// Identifies the quantization method stored in a QUANT_SEG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum QuantType {
    /// Scalar quantization (min-max per dimension).
    Scalar = 0,
    /// Product quantization (codebook per subspace).
    Product = 1,
    /// Binary threshold quantization (sign bit per dimension).
    BinaryThreshold = 2,
    /// Residual product quantization.
    ResidualPq = 3,
}

impl TryFrom<u8> for QuantType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Scalar),
            1 => Ok(Self::Product),
            2 => Ok(Self::BinaryThreshold),
            3 => Ok(Self::ResidualPq),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for raw in 0..=3u8 {
            let qt = QuantType::try_from(raw).unwrap();
            assert_eq!(qt as u8, raw);
        }
    }

    #[test]
    fn invalid_value() {
        assert_eq!(QuantType::try_from(4), Err(4));
        assert_eq!(QuantType::try_from(255), Err(255));
    }
}

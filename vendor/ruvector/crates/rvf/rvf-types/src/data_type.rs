//! Vector data type discriminator.

/// Identifies the numeric encoding of vector elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DataType {
    /// 32-bit IEEE 754 float.
    F32 = 0,
    /// 16-bit IEEE 754 half-precision float.
    F16 = 1,
    /// Brain floating point (bfloat16).
    BF16 = 2,
    /// Signed 8-bit integer (scalar quantized).
    I8 = 3,
    /// Unsigned 8-bit integer.
    U8 = 4,
    /// 4-bit integer (packed, 2 per byte).
    I4 = 5,
    /// 1-bit binary (packed, 8 per byte).
    Binary = 6,
    /// Product-quantized codes.
    PQ = 7,
    /// Custom encoding (see QUANT_SEG for details).
    Custom = 8,
}

impl DataType {
    /// Returns the number of bits per element, or `None` for variable-width types.
    pub const fn bits_per_element(self) -> Option<u32> {
        match self {
            Self::F32 => Some(32),
            Self::F16 => Some(16),
            Self::BF16 => Some(16),
            Self::I8 => Some(8),
            Self::U8 => Some(8),
            Self::I4 => Some(4),
            Self::Binary => Some(1),
            Self::PQ | Self::Custom => None,
        }
    }
}

impl TryFrom<u8> for DataType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::BF16),
            3 => Ok(Self::I8),
            4 => Ok(Self::U8),
            5 => Ok(Self::I4),
            6 => Ok(Self::Binary),
            7 => Ok(Self::PQ),
            8 => Ok(Self::Custom),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for raw in 0..=8u8 {
            let dt = DataType::try_from(raw).unwrap();
            assert_eq!(dt as u8, raw);
        }
    }

    #[test]
    fn invalid_value() {
        assert_eq!(DataType::try_from(9), Err(9));
        assert_eq!(DataType::try_from(255), Err(255));
    }

    #[test]
    fn bits_per_element() {
        assert_eq!(DataType::F32.bits_per_element(), Some(32));
        assert_eq!(DataType::F16.bits_per_element(), Some(16));
        assert_eq!(DataType::I4.bits_per_element(), Some(4));
        assert_eq!(DataType::Binary.bits_per_element(), Some(1));
        assert_eq!(DataType::PQ.bits_per_element(), None);
    }
}

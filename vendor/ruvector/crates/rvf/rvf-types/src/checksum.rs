//! Checksum / hash algorithm identifiers.

/// Identifies the hash algorithm used for segment content verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ChecksumAlgo {
    /// CRC32C (SSE4.2 hardware-accelerated). Output: 4 bytes, zero-padded to 16.
    Crc32c = 0,
    /// XXH3-128. Output: 16 bytes. Fast, good distribution.
    Xxh3_128 = 1,
    /// SHAKE-256 (first 128 bits). Post-quantum safe, cryptographic.
    Shake256 = 2,
}

impl TryFrom<u8> for ChecksumAlgo {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Crc32c),
            1 => Ok(Self::Xxh3_128),
            2 => Ok(Self::Shake256),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        assert_eq!(ChecksumAlgo::try_from(0), Ok(ChecksumAlgo::Crc32c));
        assert_eq!(ChecksumAlgo::try_from(1), Ok(ChecksumAlgo::Xxh3_128));
        assert_eq!(ChecksumAlgo::try_from(2), Ok(ChecksumAlgo::Shake256));
    }

    #[test]
    fn invalid_value() {
        assert_eq!(ChecksumAlgo::try_from(3), Err(3));
    }
}

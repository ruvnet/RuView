//! Signature algorithm identifiers and the signature footer struct.

/// Cryptographic signature algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
pub enum SignatureAlgo {
    /// Ed25519 (64-byte signature, classical).
    Ed25519 = 0,
    /// ML-DSA-65 (3,309-byte signature, NIST Level 3 post-quantum).
    MlDsa65 = 1,
    /// SLH-DSA-128s (7,856-byte signature, NIST Level 1 post-quantum).
    SlhDsa128s = 2,
}

impl SignatureAlgo {
    /// Expected signature byte length for this algorithm.
    pub const fn sig_length(self) -> u16 {
        match self {
            Self::Ed25519 => 64,
            Self::MlDsa65 => 3309,
            Self::SlhDsa128s => 7856,
        }
    }

    /// Whether this algorithm provides post-quantum security.
    pub const fn is_post_quantum(self) -> bool {
        match self {
            Self::Ed25519 => false,
            Self::MlDsa65 | Self::SlhDsa128s => true,
        }
    }
}

impl TryFrom<u16> for SignatureAlgo {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ed25519),
            1 => Ok(Self::MlDsa65),
            2 => Ok(Self::SlhDsa128s),
            other => Err(other),
        }
    }
}

/// The signature footer appended after a segment payload when the `SIGNED`
/// flag is set.
///
/// Wire layout (variable-length on the wire, fixed-size in memory):
/// ```text
/// Offset  Type    Field
/// 0x00    u16     sig_algo
/// 0x02    u16     sig_length
/// 0x04    [u8]    signature (sig_length bytes)
/// var     u32     footer_length (total footer size for backward scan)
/// ```
///
/// This struct uses a fixed-size buffer large enough for the largest
/// supported algorithm (SLH-DSA-128s = 7,856 bytes).
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignatureFooter {
    /// Signature algorithm.
    pub sig_algo: u16,
    /// Byte length of the signature.
    pub sig_length: u16,
    /// Signature bytes (only the first `sig_length` bytes are meaningful).
    pub signature: [u8; Self::MAX_SIG_LEN],
    /// Total footer size (for backward scanning).
    pub footer_length: u32,
}

impl SignatureFooter {
    /// Maximum signature length across all supported algorithms.
    pub const MAX_SIG_LEN: usize = 7856;

    /// Compute the expected footer length from the signature length.
    /// Layout: 2 (sig_algo) + 2 (sig_length) + sig_length + 4 (footer_length).
    pub const fn compute_footer_length(sig_length: u16) -> u32 {
        2 + 2 + sig_length as u32 + 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algo_round_trip() {
        for raw in 0..=2u16 {
            let a = SignatureAlgo::try_from(raw).unwrap();
            assert_eq!(a as u16, raw);
        }
        assert_eq!(SignatureAlgo::try_from(3), Err(3));
    }

    #[test]
    fn sig_lengths() {
        assert_eq!(SignatureAlgo::Ed25519.sig_length(), 64);
        assert_eq!(SignatureAlgo::MlDsa65.sig_length(), 3309);
        assert_eq!(SignatureAlgo::SlhDsa128s.sig_length(), 7856);
    }

    #[test]
    fn post_quantum_flag() {
        assert!(!SignatureAlgo::Ed25519.is_post_quantum());
        assert!(SignatureAlgo::MlDsa65.is_post_quantum());
        assert!(SignatureAlgo::SlhDsa128s.is_post_quantum());
    }

    #[test]
    fn footer_length_computation() {
        // Ed25519: 2 + 2 + 64 + 4 = 72
        assert_eq!(SignatureFooter::compute_footer_length(64), 72);
        // ML-DSA-65: 2 + 2 + 3309 + 4 = 3317
        assert_eq!(SignatureFooter::compute_footer_length(3309), 3317);
    }
}

//! SHAKE-256 hashing for cryptographic witness and content hashing.

use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

use alloc::vec;
use alloc::vec::Vec;

/// Compute SHAKE-256 hash of `data` with arbitrary `output_len`.
pub fn shake256_hash(data: &[u8], output_len: usize) -> Vec<u8> {
    let mut hasher = Shake256::default();
    hasher.update(data);
    let mut reader = hasher.finalize_xof();
    let mut output = vec![0u8; output_len];
    reader.read(&mut output);
    output
}

/// Compute 128-bit (16-byte) SHAKE-256 hash.
pub fn shake256_128(data: &[u8]) -> [u8; 16] {
    let mut hasher = Shake256::default();
    hasher.update(data);
    let mut reader = hasher.finalize_xof();
    let mut output = [0u8; 16];
    reader.read(&mut output);
    output
}

/// Compute 256-bit (32-byte) SHAKE-256 hash.
pub fn shake256_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Shake256::default();
    hasher.update(data);
    let mut reader = hasher.finalize_xof();
    let mut output = [0u8; 32];
    reader.read(&mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake256_empty_input() {
        let h128 = shake256_128(b"");
        let h256 = shake256_256(b"");
        // Non-zero output for empty input (SHAKE-256 is a sponge)
        assert_ne!(h128, [0u8; 16]);
        assert_ne!(h256, [0u8; 32]);
    }

    #[test]
    fn shake256_deterministic() {
        let a = shake256_256(b"test data");
        let b = shake256_256(b"test data");
        assert_eq!(a, b);
    }

    #[test]
    fn shake256_different_inputs() {
        let a = shake256_256(b"input A");
        let b = shake256_256(b"input B");
        assert_ne!(a, b);
    }

    #[test]
    fn shake256_arbitrary_output_len() {
        let h = shake256_hash(b"hello", 64);
        assert_eq!(h.len(), 64);
        // Prefix should match the 32-byte version
        let h32 = shake256_hash(b"hello", 32);
        assert_eq!(&h[..32], &h32[..]);
    }

    #[test]
    fn shake256_128_is_prefix_of_256() {
        let h128 = shake256_128(b"consistency check");
        let h256 = shake256_256(b"consistency check");
        assert_eq!(&h128[..], &h256[..16]);
    }

    #[test]
    fn shake256_known_vector() {
        // NIST test: SHAKE256("") first 32 bytes
        let h = shake256_hash(b"", 32);
        assert_eq!(
            h,
            [
                0x46, 0xb9, 0xdd, 0x2b, 0x0b, 0xa8, 0x8d, 0x13, 0x23, 0x3b, 0x3f, 0xeb, 0x74, 0x3e,
                0xeb, 0x24, 0x3f, 0xcd, 0x52, 0xea, 0x62, 0xb8, 0x1b, 0x82, 0xb5, 0x0c, 0x27, 0x64,
                0x6e, 0xd5, 0x76, 0x2f,
            ]
        );
    }
}

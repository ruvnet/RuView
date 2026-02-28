//! Signature footer codec for RVF segments.
//!
//! Encodes/decodes `rvf_types::SignatureFooter` to/from wire-format bytes.
//! Wire layout:
//!   [0..2]   sig_algo   (u16 LE)
//!   [2..4]   sig_length (u16 LE)
//!   [4..4+sig_length]   signature bytes
//!   [4+sig_length..4+sig_length+4]  footer_length (u32 LE)

use alloc::vec::Vec;
use rvf_types::{ErrorCode, RvfError, SignatureFooter};

/// Minimum footer wire size: 2 (algo) + 2 (sig_len) + 4 (footer_len) = 8 bytes.
const FOOTER_MIN_SIZE: usize = 8;

/// Encode a `SignatureFooter` into wire-format bytes.
pub fn encode_signature_footer(footer: &SignatureFooter) -> Vec<u8> {
    let sig_len = footer.sig_length as usize;
    let total = 2 + 2 + sig_len + 4;
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&footer.sig_algo.to_le_bytes());
    buf.extend_from_slice(&footer.sig_length.to_le_bytes());
    buf.extend_from_slice(&footer.signature[..sig_len]);
    buf.extend_from_slice(&footer.footer_length.to_le_bytes());
    buf
}

/// Decode a `SignatureFooter` from wire-format bytes.
pub fn decode_signature_footer(data: &[u8]) -> Result<SignatureFooter, RvfError> {
    if data.len() < FOOTER_MIN_SIZE {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }
    let sig_algo = u16::from_le_bytes([data[0], data[1]]);
    let sig_length = u16::from_le_bytes([data[2], data[3]]);
    let sig_len = sig_length as usize;

    if sig_len > SignatureFooter::MAX_SIG_LEN {
        return Err(RvfError::Code(ErrorCode::InvalidSignature));
    }
    if data.len() < 4 + sig_len + 4 {
        return Err(RvfError::Code(ErrorCode::TruncatedSegment));
    }

    let mut signature = [0u8; SignatureFooter::MAX_SIG_LEN];
    signature[..sig_len].copy_from_slice(&data[4..4 + sig_len]);

    let fl_offset = 4 + sig_len;
    let footer_length = u32::from_le_bytes([
        data[fl_offset],
        data[fl_offset + 1],
        data[fl_offset + 2],
        data[fl_offset + 3],
    ]);

    Ok(SignatureFooter {
        sig_algo,
        sig_length,
        signature,
        footer_length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_footer(algo: u16, sig_len: u16, fill: u8) -> SignatureFooter {
        let mut signature = [0u8; SignatureFooter::MAX_SIG_LEN];
        signature[..sig_len as usize].fill(fill);
        SignatureFooter {
            sig_algo: algo,
            sig_length: sig_len,
            signature,
            footer_length: SignatureFooter::compute_footer_length(sig_len),
        }
    }

    #[test]
    fn round_trip_ed25519() {
        let footer = make_footer(0, 64, 0xAB);
        let encoded = encode_signature_footer(&footer);
        assert_eq!(encoded.len(), 2 + 2 + 64 + 4);
        let decoded = decode_signature_footer(&encoded).unwrap();
        assert_eq!(decoded.sig_algo, footer.sig_algo);
        assert_eq!(decoded.sig_length, footer.sig_length);
        assert_eq!(&decoded.signature[..64], &footer.signature[..64]);
        assert_eq!(decoded.footer_length, footer.footer_length);
    }

    #[test]
    fn decode_truncated_header() {
        let result = decode_signature_footer(&[0u8; 5]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_truncated_signature() {
        let footer = make_footer(0, 64, 0xCC);
        let encoded = encode_signature_footer(&footer);
        let result = decode_signature_footer(&encoded[..10]);
        assert!(result.is_err());
    }

    #[test]
    fn empty_signature() {
        let footer = make_footer(1, 0, 0);
        let encoded = encode_signature_footer(&footer);
        assert_eq!(encoded.len(), FOOTER_MIN_SIZE);
        let decoded = decode_signature_footer(&encoded).unwrap();
        assert_eq!(decoded.sig_algo, 1);
        assert_eq!(decoded.sig_length, 0);
    }
}

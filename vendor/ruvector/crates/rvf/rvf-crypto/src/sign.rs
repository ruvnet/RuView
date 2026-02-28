//! Ed25519 segment signing and verification.
//!
//! Signs the canonical representation: header bytes || content_hash || context.
//! ML-DSA-65 is a future TODO behind a feature flag.

use alloc::vec::Vec;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rvf_types::{SegmentHeader, SignatureFooter};

use crate::hash::shake256_128;

/// Ed25519 algorithm identifier (matches `SignatureAlgo::Ed25519`).
const SIG_ALGO_ED25519: u16 = 0;

/// Build the canonical message to sign for a segment.
///
/// signed_data = segment_header_bytes[0..40] || content_hash || context_string || segment_id
fn build_signed_data(header: &SegmentHeader, payload: &[u8]) -> Vec<u8> {
    // Safe serialization of header fields to bytes, matching the wire format
    // layout (see write_path.rs header_to_bytes). Avoids unsafe transmute which
    // relies on compiler-specific struct layout guarantees.
    let header_bytes = header_to_sign_bytes(header);

    let mut msg = Vec::with_capacity(40 + 16 + 32);
    // First 40 bytes of header (up to but not including content_hash at offset 0x28)
    msg.extend_from_slice(&header_bytes[..40]);
    // Content hash from header
    msg.extend_from_slice(&header.content_hash);
    // Context string for domain separation
    msg.extend_from_slice(b"RVF-v1-segment");
    // Segment ID bytes for replay prevention
    msg.extend_from_slice(&header.segment_id.to_le_bytes());
    // Include payload hash for binding
    let payload_hash = shake256_128(payload);
    msg.extend_from_slice(&payload_hash);
    msg
}

/// Safely serialize a `SegmentHeader` into its 64-byte wire representation.
///
/// This mirrors the layout in `write_path::header_to_bytes` but lives here to
/// avoid an unsafe `transmute` / pointer cast whose correctness depends on
/// padding and alignment guarantees that are not enforced by the language.
fn header_to_sign_bytes(h: &SegmentHeader) -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[0x00..0x04].copy_from_slice(&h.magic.to_le_bytes());
    buf[0x04] = h.version;
    buf[0x05] = h.seg_type;
    buf[0x06..0x08].copy_from_slice(&h.flags.to_le_bytes());
    buf[0x08..0x10].copy_from_slice(&h.segment_id.to_le_bytes());
    buf[0x10..0x18].copy_from_slice(&h.payload_length.to_le_bytes());
    buf[0x18..0x20].copy_from_slice(&h.timestamp_ns.to_le_bytes());
    buf[0x20] = h.checksum_algo;
    buf[0x21] = h.compression;
    buf[0x22..0x24].copy_from_slice(&h.reserved_0.to_le_bytes());
    buf[0x24..0x28].copy_from_slice(&h.reserved_1.to_le_bytes());
    buf[0x28..0x38].copy_from_slice(&h.content_hash);
    buf[0x38..0x3C].copy_from_slice(&h.uncompressed_len.to_le_bytes());
    buf[0x3C..0x40].copy_from_slice(&h.alignment_pad.to_le_bytes());
    buf
}

/// Sign a segment with Ed25519, producing a `SignatureFooter`.
pub fn sign_segment(header: &SegmentHeader, payload: &[u8], key: &SigningKey) -> SignatureFooter {
    let msg = build_signed_data(header, payload);
    let sig: Signature = key.sign(&msg);
    let sig_bytes = sig.to_bytes();

    let mut signature = [0u8; SignatureFooter::MAX_SIG_LEN];
    signature[..64].copy_from_slice(&sig_bytes);

    SignatureFooter {
        sig_algo: SIG_ALGO_ED25519,
        sig_length: 64,
        signature,
        footer_length: SignatureFooter::compute_footer_length(64),
    }
}

/// Verify a segment signature using Ed25519.
///
/// Returns `true` if the signature is valid, `false` otherwise.
pub fn verify_segment(
    header: &SegmentHeader,
    payload: &[u8],
    footer: &SignatureFooter,
    pubkey: &VerifyingKey,
) -> bool {
    if footer.sig_algo != SIG_ALGO_ED25519 {
        return false;
    }
    if footer.sig_length != 64 {
        return false;
    }
    let msg = build_signed_data(header, payload);
    let sig_bytes: [u8; 64] = match footer.signature[..64].try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(&sig_bytes);
    pubkey.verify(&msg, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn make_test_header() -> SegmentHeader {
        let mut h = SegmentHeader::new(0x01, 42);
        h.timestamp_ns = 1_000_000_000;
        h.payload_length = 100;
        h
    }

    #[test]
    fn sign_verify_round_trip() {
        let key = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let payload = b"test payload data for signing";

        let footer = sign_segment(&header, payload, &key);
        let pubkey = key.verifying_key();

        assert!(verify_segment(&header, payload, &footer, &pubkey));
    }

    #[test]
    fn tampered_payload_fails() {
        let key = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let payload = b"original payload";

        let footer = sign_segment(&header, payload, &key);
        let pubkey = key.verifying_key();

        let tampered = b"tampered payload";
        assert!(!verify_segment(&header, tampered, &footer, &pubkey));
    }

    #[test]
    fn tampered_header_fails() {
        let key = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let payload = b"payload";

        let footer = sign_segment(&header, payload, &key);
        let pubkey = key.verifying_key();

        let mut bad_header = header;
        bad_header.segment_id = 999;
        assert!(!verify_segment(&bad_header, payload, &footer, &pubkey));
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = SigningKey::generate(&mut OsRng);
        let key2 = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let payload = b"payload";

        let footer = sign_segment(&header, payload, &key1);
        let wrong_pubkey = key2.verifying_key();

        assert!(!verify_segment(&header, payload, &footer, &wrong_pubkey));
    }

    #[test]
    fn sig_algo_is_ed25519() {
        let key = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let footer = sign_segment(&header, b"x", &key);
        assert_eq!(footer.sig_algo, 0);
        assert_eq!(footer.sig_length, 64);
    }

    #[test]
    fn footer_length_correct() {
        let key = SigningKey::generate(&mut OsRng);
        let header = make_test_header();
        let footer = sign_segment(&header, b"data", &key);
        assert_eq!(
            footer.footer_length,
            SignatureFooter::compute_footer_length(64)
        );
    }
}

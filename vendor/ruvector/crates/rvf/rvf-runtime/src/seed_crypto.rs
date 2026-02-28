//! Cryptographic operations for QR seed signing and verification.
//!
//! Uses the built-in SHA-256 and HMAC-SHA256 from rvf-types — zero dependencies.
//!
//! Signature scheme: HMAC-SHA256 with sig_algo=2.
//! Content integrity: SHA-256 truncated to 8 or 16 bytes.

use rvf_types::sha256::{ct_eq, hmac_sha256, sha256};

/// Signature algorithm ID for HMAC-SHA256 (built-in, zero-dep).
pub const SIG_ALGO_HMAC_SHA256: u16 = 2;

/// Signature algorithm ID for Ed25519 (RFC 8032 asymmetric signing).
#[cfg(feature = "ed25519")]
pub const SIG_ALGO_ED25519: u16 = 0;

/// Compute the 8-byte content hash for SeedHeader.content_hash.
///
/// SHA-256 of the data payload (microkernel + manifest) truncated to 64 bits.
pub fn seed_content_hash(data: &[u8]) -> [u8; 8] {
    let full = sha256(data);
    let mut out = [0u8; 8];
    out.copy_from_slice(&full[..8]);
    out
}

/// Compute a 16-byte layer content hash.
///
/// SHA-256 of the layer data truncated to 128 bits.
/// Used for LayerEntry.content_hash verification.
pub fn layer_content_hash(data: &[u8]) -> [u8; 16] {
    let full = sha256(data);
    let mut out = [0u8; 16];
    out.copy_from_slice(&full[..16]);
    out
}

/// Compute the full 32-byte content hash.
pub fn full_content_hash(data: &[u8]) -> [u8; 32] {
    sha256(data)
}

/// Sign a seed payload using HMAC-SHA256.
///
/// The signature covers the unsigned payload (header + microkernel + manifest).
/// Returns a 32-byte HMAC-SHA256 tag.
pub fn sign_seed(key: &[u8], payload: &[u8]) -> [u8; 32] {
    hmac_sha256(key, payload)
}

/// Verify a seed signature using HMAC-SHA256.
///
/// Uses constant-time comparison to prevent timing side channels.
pub fn verify_seed(key: &[u8], payload: &[u8], signature: &[u8]) -> bool {
    if signature.len() != 32 {
        return false;
    }
    let expected = hmac_sha256(key, payload);
    let mut sig_arr = [0u8; 32];
    sig_arr.copy_from_slice(signature);
    ct_eq(&expected, &sig_arr)
}

/// Verify a layer's content hash matches its data.
pub fn verify_layer(expected_hash: &[u8; 16], layer_data: &[u8]) -> bool {
    let computed = layer_content_hash(layer_data);
    computed == *expected_hash
}

/// Verify the seed's 8-byte content hash against payload data.
pub fn verify_content_hash(expected: &[u8; 8], data: &[u8]) -> bool {
    let computed = seed_content_hash(data);
    computed == *expected
}

// ---------------------------------------------------------------------------
// Ed25519 asymmetric signing (feature-gated)
// ---------------------------------------------------------------------------

/// Sign a seed payload using Ed25519 (RFC 8032).
///
/// Takes a 32-byte secret key and returns a 64-byte signature.
/// This is asymmetric: only the holder of the secret key can sign,
/// but anyone with the corresponding public key can verify.
#[cfg(feature = "ed25519")]
pub fn sign_seed_ed25519(secret_key: &[u8; 32], payload: &[u8]) -> [u8; 64] {
    rvf_types::ed25519::ed25519_sign(secret_key, payload)
}

/// Verify a seed signature using Ed25519 (RFC 8032).
///
/// Takes a 32-byte public key and a 64-byte signature.
/// Returns `true` if the signature is valid for the given payload.
#[cfg(feature = "ed25519")]
pub fn verify_seed_ed25519(public_key: &[u8; 32], payload: &[u8], signature: &[u8]) -> bool {
    if signature.len() != 64 {
        return false;
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(signature);
    rvf_types::ed25519::ed25519_verify(public_key, payload, &sig_arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_deterministic() {
        let data = b"test microkernel data";
        let h1 = seed_content_hash(data);
        let h2 = seed_content_hash(data);
        assert_eq!(h1, h2);
        assert_ne!(h1, [0u8; 8]);
    }

    #[test]
    fn layer_hash_deterministic() {
        let data = b"layer data block";
        let h1 = layer_content_hash(data);
        let h2 = layer_content_hash(data);
        assert_eq!(h1, h2);
        assert_ne!(h1, [0u8; 16]);
    }

    #[test]
    fn sign_verify_round_trip() {
        let key = b"my-secret-signing-key-1234567890";
        let payload = b"RVQS header + microkernel + manifest bytes";
        let sig = sign_seed(key, payload);
        assert!(verify_seed(key, payload, &sig));
    }

    #[test]
    fn wrong_key_fails() {
        let key = b"correct-key";
        let payload = b"some payload";
        let sig = sign_seed(key, payload);
        assert!(!verify_seed(b"wrong-key!!", payload, &sig));
    }

    #[test]
    fn tampered_payload_fails() {
        let key = b"signing-key";
        let payload = b"original payload";
        let sig = sign_seed(key, payload);
        assert!(!verify_seed(key, b"tampered payload", &sig));
    }

    #[test]
    fn short_signature_fails() {
        let key = b"key";
        assert!(!verify_seed(key, b"data", &[0u8; 16])); // Too short.
    }

    #[test]
    fn verify_layer_correct() {
        let data = vec![0x42u8; 4096];
        let hash = layer_content_hash(&data);
        assert!(verify_layer(&hash, &data));
    }

    #[test]
    fn verify_layer_tampered() {
        let data = vec![0x42u8; 4096];
        let hash = layer_content_hash(&data);
        let tampered = vec![0x43u8; 4096];
        assert!(!verify_layer(&hash, &tampered));
    }

    #[test]
    fn verify_content_hash_correct() {
        let data = b"microkernel + manifest";
        let hash = seed_content_hash(data);
        assert!(verify_content_hash(&hash, data));
    }

    #[test]
    fn different_data_different_hashes() {
        let h1 = seed_content_hash(b"data1");
        let h2 = seed_content_hash(b"data2");
        assert_ne!(h1, h2);
    }

    // --- Ed25519 tests (feature-gated) ---

    #[cfg(feature = "ed25519")]
    mod ed25519_tests {
        use super::*;

        /// Build a deterministic keypair for tests.
        fn test_keypair() -> ([u8; 32], [u8; 32]) {
            let secret = [0x42u8; 32];
            let kp = rvf_types::ed25519::Ed25519Keypair::from_secret(&secret);
            (kp.secret_key(), kp.public_key())
        }

        #[test]
        fn ed25519_sign_verify_round_trip() {
            let (secret, public) = test_keypair();
            let payload = b"RVQS header + microkernel + manifest bytes";
            let sig = sign_seed_ed25519(&secret, payload);
            assert!(verify_seed_ed25519(&public, payload, &sig));
        }

        #[test]
        fn ed25519_wrong_key_rejects() {
            let (secret, _public) = test_keypair();
            let other_secret = [0x99u8; 32];
            let other_kp = rvf_types::ed25519::Ed25519Keypair::from_secret(&other_secret);
            let payload = b"some payload";
            let sig = sign_seed_ed25519(&secret, payload);
            assert!(!verify_seed_ed25519(&other_kp.public_key(), payload, &sig));
        }

        #[test]
        fn ed25519_tampered_payload_rejects() {
            let (secret, public) = test_keypair();
            let payload = b"original payload";
            let sig = sign_seed_ed25519(&secret, payload);
            assert!(!verify_seed_ed25519(&public, b"tampered payload", &sig));
        }

        #[test]
        fn ed25519_short_signature_rejects() {
            let (_secret, public) = test_keypair();
            assert!(!verify_seed_ed25519(&public, b"data", &[0u8; 16]));
        }

        #[test]
        fn ed25519_algo_constant() {
            assert_eq!(SIG_ALGO_ED25519, 0);
        }
    }
}

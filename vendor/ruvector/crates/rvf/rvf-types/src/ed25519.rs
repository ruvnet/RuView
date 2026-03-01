//! Ed25519 asymmetric signing (RFC 8032).
//!
//! Provides keypair generation, signing, and verification using the
//! `ed25519-dalek` crate. Feature-gated behind the `ed25519` feature.

use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey, Verifier, VerifyingKey};

/// Ed25519 public key size in bytes.
pub const PUBLIC_KEY_SIZE: usize = 32;

/// Ed25519 secret (signing) key size in bytes.
pub const SECRET_KEY_SIZE: usize = 32;

/// Ed25519 signature size in bytes.
pub const SIGNATURE_SIZE: usize = 64;

// Compile-time size assertions (mirrors sha256.rs pattern).
const _: () = assert!(PUBLIC_KEY_SIZE == 32);
const _: () = assert!(SECRET_KEY_SIZE == 32);
const _: () = assert!(SIGNATURE_SIZE == 64);

/// An Ed25519 keypair (signing key + verifying key).
///
/// The signing key is 32 bytes of secret material; the verifying key
/// is the corresponding 32-byte public point on the Ed25519 curve.
#[derive(Clone)]
pub struct Ed25519Keypair {
    signing: SigningKey,
}

impl Ed25519Keypair {
    /// Generate a new random keypair from the provided RNG.
    pub fn generate<R: rand_core::CryptoRngCore>(rng: &mut R) -> Self {
        Self {
            signing: SigningKey::generate(rng),
        }
    }

    /// Reconstruct a keypair from a 32-byte secret key.
    pub fn from_secret(secret: &[u8; SECRET_KEY_SIZE]) -> Self {
        Self {
            signing: SigningKey::from_bytes(secret),
        }
    }

    /// Return the 32-byte secret (signing) key.
    pub fn secret_key(&self) -> [u8; SECRET_KEY_SIZE] {
        self.signing.to_bytes()
    }

    /// Return the 32-byte public (verifying) key.
    pub fn public_key(&self) -> [u8; PUBLIC_KEY_SIZE] {
        self.signing.verifying_key().to_bytes()
    }
}

/// Sign `message` with an Ed25519 secret key. Returns a 64-byte signature.
///
/// This is a deterministic operation: the same key + message always
/// produces the same signature (per RFC 8032).
pub fn ed25519_sign(secret: &[u8; SECRET_KEY_SIZE], message: &[u8]) -> [u8; SIGNATURE_SIZE] {
    let signing_key = SigningKey::from_bytes(secret);
    let sig: DalekSignature = signing_key.sign(message);
    sig.to_bytes()
}

/// Verify an Ed25519 signature against a public key and message.
///
/// Returns `true` if the signature is valid, `false` otherwise.
pub fn ed25519_verify(
    public: &[u8; PUBLIC_KEY_SIZE],
    message: &[u8],
    signature: &[u8; SIGNATURE_SIZE],
) -> bool {
    let Ok(verifying_key) = VerifyingKey::from_bytes(public) else {
        return false;
    };
    let sig = DalekSignature::from_bytes(signature);
    verifying_key.verify(message, &sig).is_ok()
}

/// Constant-time comparison of two 64-byte signatures.
pub fn ct_eq_sig(a: &[u8; SIGNATURE_SIZE], b: &[u8; SIGNATURE_SIZE]) -> bool {
    let mut diff = 0u8;
    for i in 0..SIGNATURE_SIZE {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic RNG for reproducible tests.
    struct TestRng(u64);

    impl rand_core::RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            // Simple xorshift64 for test determinism.
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            let mut i = 0;
            while i < dest.len() {
                let val = self.next_u64().to_le_bytes();
                let remaining = dest.len() - i;
                let take = if remaining < 8 { remaining } else { 8 };
                dest[i..i + take].copy_from_slice(&val[..take]);
                i += take;
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    impl rand_core::CryptoRng for TestRng {}

    fn test_rng() -> TestRng {
        TestRng(0xDEAD_BEEF_CAFE_1234)
    }

    // --- Test 1: keypair generation ---

    #[test]
    fn keygen_produces_valid_keypair() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);

        let secret = kp.secret_key();
        let public = kp.public_key();

        assert_eq!(secret.len(), SECRET_KEY_SIZE);
        assert_eq!(public.len(), PUBLIC_KEY_SIZE);
        // Keys should not be all zeros.
        assert_ne!(secret, [0u8; SECRET_KEY_SIZE]);
        assert_ne!(public, [0u8; PUBLIC_KEY_SIZE]);
    }

    // --- Test 2: sign produces a signature ---

    #[test]
    fn sign_returns_64_byte_signature() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let message = b"hello RVF";

        let sig = ed25519_sign(&kp.secret_key(), message);
        assert_eq!(sig.len(), SIGNATURE_SIZE);
        assert_ne!(sig, [0u8; SIGNATURE_SIZE]);
    }

    // --- Test 3: sign then verify round-trip ---

    #[test]
    fn sign_verify_round_trip() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let message = b"The quick brown fox jumps over the lazy dog";

        let sig = ed25519_sign(&kp.secret_key(), message);
        assert!(ed25519_verify(&kp.public_key(), message, &sig));
    }

    // --- Test 4: wrong key rejects ---

    #[test]
    fn wrong_key_rejects() {
        let mut rng = test_rng();
        let kp1 = Ed25519Keypair::generate(&mut rng);
        let kp2 = Ed25519Keypair::generate(&mut rng);
        let message = b"signed by kp1";

        let sig = ed25519_sign(&kp1.secret_key(), message);
        // Verify with kp2's public key should fail.
        assert!(!ed25519_verify(&kp2.public_key(), message, &sig));
    }

    // --- Test 5: tampered message rejects ---

    #[test]
    fn tampered_message_rejects() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let message = b"original payload";

        let sig = ed25519_sign(&kp.secret_key(), message);
        assert!(!ed25519_verify(&kp.public_key(), b"tampered payload", &sig));
    }

    // --- Test 6: deterministic signatures ---

    #[test]
    fn deterministic_signatures() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let message = b"determinism test";

        let sig1 = ed25519_sign(&kp.secret_key(), message);
        let sig2 = ed25519_sign(&kp.secret_key(), message);
        assert_eq!(sig1, sig2);
    }

    // --- Test 7: different messages produce different signatures ---

    #[test]
    fn different_messages_different_sigs() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);

        let sig_a = ed25519_sign(&kp.secret_key(), b"message A");
        let sig_b = ed25519_sign(&kp.secret_key(), b"message B");
        assert_ne!(sig_a, sig_b);
    }

    // --- Test 8: empty message ---

    #[test]
    fn empty_message_sign_verify() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let message = b"";

        let sig = ed25519_sign(&kp.secret_key(), message);
        assert!(ed25519_verify(&kp.public_key(), message, &sig));
    }

    // --- Additional tests ---

    #[test]
    fn from_secret_round_trip() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);

        let secret = kp.secret_key();
        let restored = Ed25519Keypair::from_secret(&secret);

        assert_eq!(kp.public_key(), restored.public_key());
        assert_eq!(kp.secret_key(), restored.secret_key());
    }

    #[test]
    fn ct_eq_sig_same() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let sig = ed25519_sign(&kp.secret_key(), b"test");
        assert!(ct_eq_sig(&sig, &sig));
    }

    #[test]
    fn ct_eq_sig_different() {
        let mut rng = test_rng();
        let kp = Ed25519Keypair::generate(&mut rng);
        let sig1 = ed25519_sign(&kp.secret_key(), b"msg1");
        let sig2 = ed25519_sign(&kp.secret_key(), b"msg2");
        assert!(!ct_eq_sig(&sig1, &sig2));
    }
}

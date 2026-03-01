//! Pi-Key: Ultra-compact WASM-based cryptographic key system
//!
//! Uses mathematical constants (Pi, e, φ) for key sizing to encode purpose:
//! - Pi (314 bits) = Identity keys
//! - e (271 bits) = Ephemeral/session keys
//! - φ (161 bits) = Genesis/origin keys
//!
//! The key sizes are derived from mathematical constants:
//! - Pi: 3.14159... → 314 bits (39.25 bytes → 40 bytes)
//! - Euler's e: 2.71828... → 271 bits (33.875 bytes → 34 bytes)
//! - Golden ratio φ: 1.61803... → 161 bits (20.125 bytes → 21 bytes)
//!
//! This creates ultra-compact, semantically meaningful keys.

use wasm_bindgen::prelude::*;
use sha2::{Sha256, Sha512, Digest};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::{RngCore, rngs::OsRng};
use serde::{Serialize, Deserialize};
use argon2::{Argon2, Algorithm, Version, Params, password_hash::SaltString};
use zeroize::Zeroize;

/// Mathematical constant key sizes (in bits)
pub mod sizes {
    /// Pi-key: 314 bits (40 bytes) - Primary identity keys
    pub const PI_BITS: usize = 314;
    pub const PI_BYTES: usize = 40;

    /// Euler-key: 271 bits (34 bytes) - Ephemeral/session keys
    pub const EULER_BITS: usize = 271;
    pub const EULER_BYTES: usize = 34;

    /// Golden ratio key: 161 bits (21 bytes) - Genesis/compact keys
    pub const PHI_BITS: usize = 161;
    pub const PHI_BYTES: usize = 21;

    /// Combined key: 746 bits (94 bytes) = π + e + φ
    pub const COMBINED_BYTES: usize = 94;

    /// Verification constant: First 16 digits of Pi as hex
    pub const PI_MAGIC: [u8; 8] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x58, 0x97, 0x93];
}

/// Key purpose encoded by size
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum KeyPurpose {
    /// Pi-sized: Primary identity (314 bits)
    Identity,
    /// Euler-sized: Session/ephemeral (271 bits)
    Ephemeral,
    /// Phi-sized: Genesis/origin (161 bits)
    Genesis,
    /// Unknown/custom size
    Custom(usize),
}

impl KeyPurpose {
    pub fn size_bytes(&self) -> usize {
        match self {
            KeyPurpose::Identity => sizes::PI_BYTES,
            KeyPurpose::Ephemeral => sizes::EULER_BYTES,
            KeyPurpose::Genesis => sizes::PHI_BYTES,
            KeyPurpose::Custom(n) => *n,
        }
    }

    pub fn from_size(size: usize) -> Self {
        match size {
            sizes::PI_BYTES => KeyPurpose::Identity,
            sizes::EULER_BYTES => KeyPurpose::Ephemeral,
            sizes::PHI_BYTES => KeyPurpose::Genesis,
            n => KeyPurpose::Custom(n),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            KeyPurpose::Identity => "π",
            KeyPurpose::Ephemeral => "e",
            KeyPurpose::Genesis => "φ",
            KeyPurpose::Custom(_) => "?",
        }
    }
}

/// Ultra-compact Pi-Key (40 bytes identity + 21 bytes genesis signature)
#[wasm_bindgen]
pub struct PiKey {
    /// Identity key (Pi-sized: 40 bytes)
    identity: [u8; sizes::PI_BYTES],
    /// Private signing key (Ed25519)
    #[wasm_bindgen(skip)]
    signing_key: SigningKey,
    /// Genesis fingerprint (Phi-sized: 21 bytes)
    genesis_fingerprint: [u8; sizes::PHI_BYTES],
    /// Encrypted backup (AES-256-GCM)
    #[wasm_bindgen(skip)]
    encrypted_backup: Option<Vec<u8>>,
}

/// Compact serializable key format
#[derive(Serialize, Deserialize)]
struct CompactKeyFormat {
    /// Version byte
    version: u8,
    /// Purpose marker (derived from size)
    purpose: KeyPurpose,
    /// Pi magic header for validation
    magic: [u8; 8],
    /// Key material
    key: Vec<u8>,
    /// Genesis link (if applicable)
    genesis_link: Option<[u8; sizes::PHI_BYTES]>,
    /// Creation timestamp
    created_at: u64,
}

#[wasm_bindgen]
impl PiKey {
    /// Generate a new Pi-Key with genesis linking
    #[wasm_bindgen(constructor)]
    pub fn generate(genesis_seed: Option<Vec<u8>>) -> Result<PiKey, JsValue> {
        let mut csprng = OsRng;

        // Generate Ed25519 signing key
        let signing_key = SigningKey::generate(&mut csprng);

        // Derive Pi-sized identity from public key
        let verifying_key = VerifyingKey::from(&signing_key);
        let identity = Self::derive_pi_identity(&verifying_key);

        // Create genesis fingerprint
        let genesis_fingerprint = match genesis_seed {
            Some(seed) => Self::derive_genesis_fingerprint(&seed),
            None => Self::derive_genesis_fingerprint(identity.as_slice()),
        };

        Ok(PiKey {
            identity,
            signing_key,
            genesis_fingerprint,
            encrypted_backup: None,
        })
    }

    /// Derive Pi-sized (40 byte) identity from public key
    fn derive_pi_identity(verifying_key: &VerifyingKey) -> [u8; sizes::PI_BYTES] {
        let mut hasher = Sha512::new();
        hasher.update(&sizes::PI_MAGIC);
        hasher.update(verifying_key.as_bytes());
        let hash = hasher.finalize();

        let mut identity = [0u8; sizes::PI_BYTES];
        identity.copy_from_slice(&hash[..sizes::PI_BYTES]);

        // Embed Pi magic marker in first 4 bytes (after XOR to preserve entropy)
        for i in 0..4 {
            identity[i] ^= sizes::PI_MAGIC[i];
        }

        identity
    }

    /// Derive Phi-sized (21 byte) genesis fingerprint
    fn derive_genesis_fingerprint(seed: &[u8]) -> [u8; sizes::PHI_BYTES] {
        let mut hasher = Sha256::new();
        hasher.update(b"GENESIS:");
        hasher.update(&[0x16, 0x18, 0x03, 0x39]); // Golden ratio digits
        hasher.update(seed);
        let hash = hasher.finalize();

        let mut fingerprint = [0u8; sizes::PHI_BYTES];
        fingerprint.copy_from_slice(&hash[..sizes::PHI_BYTES]);
        fingerprint
    }

    /// Get the Pi-sized identity (40 bytes)
    #[wasm_bindgen(js_name = getIdentity)]
    pub fn get_identity(&self) -> Vec<u8> {
        self.identity.to_vec()
    }

    /// Get identity as hex string
    #[wasm_bindgen(js_name = getIdentityHex)]
    pub fn get_identity_hex(&self) -> String {
        hex::encode(&self.identity)
    }

    /// Get the Phi-sized genesis fingerprint (21 bytes)
    #[wasm_bindgen(js_name = getGenesisFingerprint)]
    pub fn get_genesis_fingerprint(&self) -> Vec<u8> {
        self.genesis_fingerprint.to_vec()
    }

    /// Get short identity (first 8 bytes as hex)
    #[wasm_bindgen(js_name = getShortId)]
    pub fn get_short_id(&self) -> String {
        format!("π:{}", hex::encode(&self.identity[..8]))
    }

    /// Verify this key has Pi magic marker
    #[wasm_bindgen(js_name = verifyPiMagic)]
    pub fn verify_pi_magic(&self) -> bool {
        for i in 0..4 {
            if (self.identity[i] ^ sizes::PI_MAGIC[i]) == 0 {
                return false; // Should have non-zero XOR result
            }
        }
        true
    }

    /// Sign data with this key
    #[wasm_bindgen]
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        let signature = self.signing_key.sign(data);
        signature.to_bytes().to_vec()
    }

    /// Verify signature from another Pi-Key
    #[wasm_bindgen]
    pub fn verify(&self, data: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
        if signature.len() != 64 || public_key.len() != 32 {
            return false;
        }

        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let pubkey_bytes: [u8; 32] = match public_key.try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };

        // Signature::from_bytes returns Signature directly in ed25519-dalek 2.x
        let sig = Signature::from_bytes(&sig_bytes);

        let verifying_key = match VerifyingKey::from_bytes(&pubkey_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };

        verifying_key.verify(data, &sig).is_ok()
    }

    /// Get public key for verification
    #[wasm_bindgen(js_name = getPublicKey)]
    pub fn get_public_key(&self) -> Vec<u8> {
        let verifying_key = VerifyingKey::from(&self.signing_key);
        verifying_key.as_bytes().to_vec()
    }

    /// Derive encryption key using Argon2id (memory-hard KDF)
    /// Parameters tuned for browser WASM: 64MB memory, 3 iterations
    fn derive_key_argon2id(password: &str, salt: &[u8]) -> Result<[u8; 32], JsValue> {
        // Argon2id parameters: 64MB memory, 3 iterations, 1 parallelism
        // These are tuned for browser WASM while still being secure
        let params = Params::new(
            65536,  // 64 MB memory cost
            3,      // 3 iterations (time cost)
            1,      // 1 lane (parallelism - WASM is single-threaded)
            Some(32) // 32 byte output
        ).map_err(|e| JsValue::from_str(&format!("Argon2 params error: {}", e)))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_material = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), salt, &mut key_material)
            .map_err(|e| JsValue::from_str(&format!("Argon2 error: {}", e)))?;

        Ok(key_material)
    }

    /// Create encrypted backup of private key using Argon2id KDF
    #[wasm_bindgen(js_name = createEncryptedBackup)]
    pub fn create_encrypted_backup(&mut self, password: &str) -> Result<Vec<u8>, JsValue> {
        // Generate random salt for Argon2id
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);

        // Derive encryption key using Argon2id (memory-hard, resistant to brute-force)
        let mut key_material = Self::derive_key_argon2id(password, &salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key_material)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt private key
        let plaintext = self.signing_key.as_bytes();
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
            .map_err(|e| JsValue::from_str(&format!("Encryption error: {}", e)))?;

        // Zeroize key material after use
        key_material.zeroize();

        // Combine: version (1) + purpose (1) + salt (16) + nonce (12) + ciphertext
        // Version 0x02 indicates Argon2id KDF
        let mut backup = Vec::with_capacity(2 + 16 + 12 + ciphertext.len());
        backup.push(0x02); // Version 2 = Argon2id
        backup.push(0x01); // Purpose marker: 1 = Identity (Pi-key)
        backup.extend_from_slice(&salt);
        backup.extend_from_slice(&nonce_bytes);
        backup.extend_from_slice(&ciphertext);

        self.encrypted_backup = Some(backup.clone());
        Ok(backup)
    }

    /// Restore from encrypted backup (supports both v1 legacy and v2 Argon2id)
    #[wasm_bindgen(js_name = restoreFromBackup)]
    pub fn restore_from_backup(backup: &[u8], password: &str) -> Result<PiKey, JsValue> {
        if backup.len() < 14 {
            return Err(JsValue::from_str("Backup too short"));
        }

        let version = backup[0];

        let (key_material, nonce_start, nonce_end) = match version {
            0x01 => {
                // Legacy v1: SHA-256 based (deprecated but supported for migration)
                let mut hasher = Sha256::new();
                hasher.update(password.as_bytes());
                hasher.update(&sizes::PI_MAGIC);
                let hash = hasher.finalize();
                let mut key = [0u8; 32];
                key.copy_from_slice(&hash);
                (key, 2usize, 14usize)
            },
            0x02 => {
                // v2: Argon2id (secure)
                if backup.len() < 30 {
                    return Err(JsValue::from_str("Backup too short for v2 format"));
                }
                let salt = &backup[2..18];
                let key = Self::derive_key_argon2id(password, salt)?;
                (key, 18usize, 30usize)
            },
            _ => {
                return Err(JsValue::from_str(&format!("Unknown backup version: {}", version)));
            }
        };

        let cipher = Aes256Gcm::new_from_slice(&key_material)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&backup[nonce_start..nonce_end]);
        let ciphertext = &backup[nonce_end..];

        // Decrypt
        let mut plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| JsValue::from_str("Decryption failed - wrong password?"))?;

        if plaintext.len() != 32 {
            plaintext.zeroize();
            return Err(JsValue::from_str("Invalid key length after decryption"));
        }

        let mut key_bytes: [u8; 32] = plaintext.clone().try_into()
            .map_err(|_| JsValue::from_str("Key conversion error"))?;
        plaintext.zeroize();

        let signing_key = SigningKey::from_bytes(&key_bytes);
        key_bytes.zeroize();

        let verifying_key = VerifyingKey::from(&signing_key);
        let identity = Self::derive_pi_identity(&verifying_key);
        let genesis_fingerprint = Self::derive_genesis_fingerprint(&identity);

        Ok(PiKey {
            identity,
            signing_key,
            genesis_fingerprint,
            encrypted_backup: Some(backup.to_vec()),
        })
    }

    /// Export minimal key representation (Pi + Phi sized = 61 bytes total)
    #[wasm_bindgen(js_name = exportCompact)]
    pub fn export_compact(&self) -> Vec<u8> {
        let mut compact = Vec::with_capacity(sizes::PI_BYTES + sizes::PHI_BYTES);
        compact.extend_from_slice(&self.identity);
        compact.extend_from_slice(&self.genesis_fingerprint);
        compact
    }

    /// Get key statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        format!(
            r#"{{"identity_size_bits":{}, "identity_size_bytes":{}, "genesis_size_bits":{}, "genesis_size_bytes":{}, "combined_bytes":{}, "purpose":"π-identity", "has_backup":{}}}"#,
            sizes::PI_BITS,
            sizes::PI_BYTES,
            sizes::PHI_BITS,
            sizes::PHI_BYTES,
            sizes::PI_BYTES + sizes::PHI_BYTES,
            self.encrypted_backup.is_some()
        )
    }
}

/// Genesis Key - Ultra-compact origin marker (φ-sized: 21 bytes)
#[wasm_bindgen]
pub struct GenesisKey {
    /// Phi-sized genesis identifier (21 bytes)
    id: [u8; sizes::PHI_BYTES],
    /// Creation timestamp
    created_at: u64,
    /// Network epoch
    epoch: u32,
    /// Signature from creator
    creator_signature: Vec<u8>,
}

#[wasm_bindgen]
impl GenesisKey {
    /// Create a new genesis key
    #[wasm_bindgen(constructor)]
    pub fn create(creator: &PiKey, epoch: u32) -> Result<GenesisKey, JsValue> {
        let mut hasher = Sha256::new();
        hasher.update(b"GENESIS_ORIGIN:");
        hasher.update(&[0x16, 0x18, 0x03, 0x39]); // φ
        hasher.update(&creator.identity);
        hasher.update(&epoch.to_be_bytes());
        hasher.update(&(js_sys::Date::now() as u64).to_be_bytes());
        let hash = hasher.finalize();

        let mut id = [0u8; sizes::PHI_BYTES];
        id.copy_from_slice(&hash[..sizes::PHI_BYTES]);

        let created_at = js_sys::Date::now() as u64;

        // Sign the genesis data
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&id);
        sign_data.extend_from_slice(&created_at.to_be_bytes());
        sign_data.extend_from_slice(&epoch.to_be_bytes());
        let creator_signature = creator.sign(&sign_data);

        Ok(GenesisKey {
            id,
            created_at,
            epoch,
            creator_signature,
        })
    }

    /// Get the φ-sized genesis ID
    #[wasm_bindgen(js_name = getId)]
    pub fn get_id(&self) -> Vec<u8> {
        self.id.to_vec()
    }

    /// Get ID as hex
    #[wasm_bindgen(js_name = getIdHex)]
    pub fn get_id_hex(&self) -> String {
        format!("φ:{}", hex::encode(&self.id))
    }

    /// Verify this genesis key was created by a specific Pi-Key
    #[wasm_bindgen]
    pub fn verify(&self, creator_public_key: &[u8]) -> bool {
        if creator_public_key.len() != 32 {
            return false;
        }

        let pubkey_bytes: [u8; 32] = creator_public_key.try_into().unwrap();
        let verifying_key = match VerifyingKey::from_bytes(&pubkey_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&self.id);
        sign_data.extend_from_slice(&self.created_at.to_be_bytes());
        sign_data.extend_from_slice(&self.epoch.to_be_bytes());

        if self.creator_signature.len() != 64 {
            return false;
        }

        let sig_bytes: [u8; 64] = match self.creator_signature.clone().try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        // Signature::from_bytes returns Signature directly in ed25519-dalek 2.x
        let sig = Signature::from_bytes(&sig_bytes);

        verifying_key.verify(&sign_data, &sig).is_ok()
    }

    /// Export ultra-compact genesis key (21 bytes only)
    #[wasm_bindgen(js_name = exportUltraCompact)]
    pub fn export_ultra_compact(&self) -> Vec<u8> {
        self.id.to_vec()
    }

    /// Get epoch
    #[wasm_bindgen(js_name = getEpoch)]
    pub fn get_epoch(&self) -> u32 {
        self.epoch
    }
}

/// Session Key - Euler-sized ephemeral key (e-sized: 34 bytes)
#[wasm_bindgen]
pub struct SessionKey {
    /// Euler-sized session identifier (34 bytes)
    id: [u8; sizes::EULER_BYTES],
    /// AES-256 encryption key (32 bytes, derived from id)
    #[wasm_bindgen(skip)]
    encryption_key: [u8; 32],
    /// Expiration timestamp
    expires_at: u64,
    /// Parent identity link
    parent_identity: [u8; sizes::PI_BYTES],
}

#[wasm_bindgen]
impl SessionKey {
    /// Create a new session key linked to a Pi-Key identity
    #[wasm_bindgen(constructor)]
    pub fn create(parent: &PiKey, ttl_seconds: u32) -> Result<SessionKey, JsValue> {
        let mut csprng = OsRng;
        let mut random_bytes = [0u8; 32];
        csprng.fill_bytes(&mut random_bytes);

        // Derive Euler-sized session ID
        let mut hasher = Sha512::new();
        hasher.update(b"SESSION:");
        hasher.update(&[0x27, 0x18, 0x28, 0x18]); // e digits
        hasher.update(&parent.identity);
        hasher.update(&random_bytes);
        let hash = hasher.finalize();

        let mut id = [0u8; sizes::EULER_BYTES];
        id.copy_from_slice(&hash[..sizes::EULER_BYTES]);

        // Derive encryption key
        let mut key_hasher = Sha256::new();
        key_hasher.update(&id);
        key_hasher.update(&random_bytes);
        let encryption_key: [u8; 32] = key_hasher.finalize().into();

        let expires_at = js_sys::Date::now() as u64 + (ttl_seconds as u64 * 1000);

        Ok(SessionKey {
            id,
            encryption_key,
            expires_at,
            parent_identity: parent.identity,
        })
    }

    /// Get the e-sized session ID
    #[wasm_bindgen(js_name = getId)]
    pub fn get_id(&self) -> Vec<u8> {
        self.id.to_vec()
    }

    /// Get ID as hex
    #[wasm_bindgen(js_name = getIdHex)]
    pub fn get_id_hex(&self) -> String {
        format!("e:{}", hex::encode(&self.id))
    }

    /// Check if session is expired
    #[wasm_bindgen(js_name = isExpired)]
    pub fn is_expired(&self) -> bool {
        js_sys::Date::now() as u64 > self.expires_at
    }

    /// Encrypt data with this session key
    #[wasm_bindgen]
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, JsValue> {
        if self.is_expired() {
            return Err(JsValue::from_str("Session key expired"));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| JsValue::from_str(&format!("Encryption error: {}", e)))?;

        // Return: nonce (12) + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data with this session key
    #[wasm_bindgen]
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, JsValue> {
        if data.len() < 12 {
            return Err(JsValue::from_str("Data too short"));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        cipher.decrypt(nonce, ciphertext)
            .map_err(|_| JsValue::from_str("Decryption failed"))
    }

    /// Get parent identity fingerprint
    #[wasm_bindgen(js_name = getParentIdentity)]
    pub fn get_parent_identity(&self) -> Vec<u8> {
        self.parent_identity.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_sizes() {
        assert_eq!(sizes::PI_BYTES, 40);
        assert_eq!(sizes::EULER_BYTES, 34);
        assert_eq!(sizes::PHI_BYTES, 21);
        assert_eq!(sizes::COMBINED_BYTES, 94);
    }

    #[test]
    fn test_key_purpose_from_size() {
        assert_eq!(KeyPurpose::from_size(40), KeyPurpose::Identity);
        assert_eq!(KeyPurpose::from_size(34), KeyPurpose::Ephemeral);
        assert_eq!(KeyPurpose::from_size(21), KeyPurpose::Genesis);
        assert_eq!(KeyPurpose::from_size(64), KeyPurpose::Custom(64));
    }

    #[test]
    fn test_purpose_symbols() {
        assert_eq!(KeyPurpose::Identity.symbol(), "π");
        assert_eq!(KeyPurpose::Ephemeral.symbol(), "e");
        assert_eq!(KeyPurpose::Genesis.symbol(), "φ");
    }
}

//! Node identity management with Ed25519 keypairs

use wasm_bindgen::prelude::*;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use sha2::{Sha256, Digest};
use rand::{rngs::OsRng, RngCore};
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use argon2::{Argon2, Algorithm, Version, Params};
use zeroize::Zeroize;

/// Node identity with Ed25519 keypair
#[wasm_bindgen]
pub struct WasmNodeIdentity {
    signing_key: SigningKey,
    node_id: String,
    site_id: String,
    fingerprint: Option<String>,
}

#[wasm_bindgen]
impl WasmNodeIdentity {
    /// Generate a new node identity
    #[wasm_bindgen]
    pub fn generate(site_id: &str) -> Result<WasmNodeIdentity, JsValue> {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        // Derive node ID from public key
        let verifying_key = signing_key.verifying_key();
        let node_id = Self::derive_node_id(&verifying_key);

        Ok(WasmNodeIdentity {
            signing_key,
            node_id,
            site_id: site_id.to_string(),
            fingerprint: None,
        })
    }

    /// Restore identity from secret key bytes
    #[wasm_bindgen(js_name = fromSecretKey)]
    pub fn from_secret_key(secret_key: &[u8], site_id: &str) -> Result<WasmNodeIdentity, JsValue> {
        if secret_key.len() != 32 {
            return Err(JsValue::from_str("Secret key must be 32 bytes"));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(secret_key);

        let signing_key = SigningKey::from_bytes(&key_bytes);
        let verifying_key = signing_key.verifying_key();
        let node_id = Self::derive_node_id(&verifying_key);

        Ok(WasmNodeIdentity {
            signing_key,
            node_id,
            site_id: site_id.to_string(),
            fingerprint: None,
        })
    }

    /// Get the node's unique identifier
    #[wasm_bindgen(js_name = nodeId)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }

    /// Get the site ID
    #[wasm_bindgen(js_name = siteId)]
    pub fn site_id(&self) -> String {
        self.site_id.clone()
    }

    /// Get the public key as hex string
    #[wasm_bindgen(js_name = publicKeyHex)]
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Get the public key as bytes
    #[wasm_bindgen(js_name = publicKeyBytes)]
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.signing_key.verifying_key().as_bytes().to_vec()
    }

    /// Export secret key encrypted with password (secure backup)
    /// Uses Argon2id for key derivation and AES-256-GCM for encryption
    #[wasm_bindgen(js_name = exportSecretKey)]
    pub fn export_secret_key(&self, password: &str) -> Result<Vec<u8>, JsValue> {
        if password.len() < 8 {
            return Err(JsValue::from_str("Password must be at least 8 characters"));
        }

        // Generate random salt
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);

        // Derive encryption key using Argon2id
        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|e| JsValue::from_str(&format!("Argon2 params error: {}", e)))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_material = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), &salt, &mut key_material)
            .map_err(|e| JsValue::from_str(&format!("Key derivation error: {}", e)))?;

        // Encrypt the secret key
        let cipher = Aes256Gcm::new_from_slice(&key_material)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self.signing_key.to_bytes();
        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
            .map_err(|e| JsValue::from_str(&format!("Encryption error: {}", e)))?;

        // Zeroize sensitive material
        key_material.zeroize();

        // Format: version (1) + salt (16) + nonce (12) + ciphertext
        let mut result = Vec::with_capacity(1 + 16 + 12 + ciphertext.len());
        result.push(0x01); // Version 1
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Import secret key from encrypted backup
    #[wasm_bindgen(js_name = importSecretKey)]
    pub fn import_secret_key(encrypted: &[u8], password: &str, site_id: &str) -> Result<WasmNodeIdentity, JsValue> {
        if encrypted.len() < 30 {
            return Err(JsValue::from_str("Encrypted data too short"));
        }

        let version = encrypted[0];
        if version != 0x01 {
            return Err(JsValue::from_str(&format!("Unknown version: {}", version)));
        }

        let salt = &encrypted[1..17];
        let nonce_bytes = &encrypted[17..29];
        let ciphertext = &encrypted[29..];

        // Derive decryption key
        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|e| JsValue::from_str(&format!("Argon2 params error: {}", e)))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_material = [0u8; 32];
        argon2.hash_password_into(password.as_bytes(), salt, &mut key_material)
            .map_err(|e| JsValue::from_str(&format!("Key derivation error: {}", e)))?;

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&key_material)
            .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let mut plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| JsValue::from_str("Decryption failed - wrong password?"))?;

        key_material.zeroize();

        if plaintext.len() != 32 {
            plaintext.zeroize();
            return Err(JsValue::from_str("Invalid key length"));
        }

        let mut key_bytes: [u8; 32] = plaintext.clone().try_into()
            .map_err(|_| JsValue::from_str("Key conversion error"))?;
        plaintext.zeroize();

        let signing_key = SigningKey::from_bytes(&key_bytes);
        key_bytes.zeroize();

        let verifying_key = signing_key.verifying_key();
        let node_id = Self::derive_node_id(&verifying_key);

        Ok(WasmNodeIdentity {
            signing_key,
            node_id,
            site_id: site_id.to_string(),
            fingerprint: None,
        })
    }

    /// Sign a message
    #[wasm_bindgen]
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature = self.signing_key.sign(message);
        signature.to_bytes().to_vec()
    }

    /// Verify a signature
    #[wasm_bindgen]
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        if signature.len() != 64 {
            return false;
        }

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(signature);

        match Signature::from_bytes(&sig_bytes) {
            sig => self.signing_key.verifying_key().verify(message, &sig).is_ok(),
        }
    }

    /// Verify a signature from another node
    #[wasm_bindgen(js_name = verifyFrom)]
    pub fn verify_from(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        if public_key.len() != 32 || signature.len() != 64 {
            return false;
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(public_key);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(signature);

        let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };

        let signature = Signature::from_bytes(&sig_bytes);
        verifying_key.verify(message, &signature).is_ok()
    }

    /// Set browser fingerprint for anti-sybil
    #[wasm_bindgen(js_name = setFingerprint)]
    pub fn set_fingerprint(&mut self, fingerprint: &str) {
        self.fingerprint = Some(fingerprint.to_string());
    }

    /// Get browser fingerprint
    #[wasm_bindgen(js_name = getFingerprint)]
    pub fn get_fingerprint(&self) -> Option<String> {
        self.fingerprint.clone()
    }

    /// Derive node ID from public key
    fn derive_node_id(verifying_key: &VerifyingKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifying_key.as_bytes());
        let hash = hasher.finalize();

        // Use first 16 bytes as node ID (base58 encoded)
        let mut id_bytes = [0u8; 16];
        id_bytes.copy_from_slice(&hash[..16]);

        // Simple hex encoding for now
        format!("node-{}", hex::encode(&id_bytes[..8]))
    }
}

/// Browser fingerprint generator for anti-sybil protection
#[wasm_bindgen]
pub struct BrowserFingerprint;

#[wasm_bindgen]
impl BrowserFingerprint {
    /// Generate anonymous uniqueness score
    /// This doesn't track users, just ensures one node per browser
    #[wasm_bindgen]
    pub async fn generate() -> Result<String, JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No window object"))?;

        let navigator = window.navigator();
        let screen = window.screen()
            .map_err(|_| JsValue::from_str("No screen object"))?;

        let mut components = Vec::new();

        // Hardware signals (non-identifying)
        components.push(format!("{}", navigator.hardware_concurrency()));
        components.push(format!("{}x{}", screen.width().unwrap_or(0), screen.height().unwrap_or(0)));

        // Timezone offset
        let date = js_sys::Date::new_0();
        components.push(format!("{}", date.get_timezone_offset()));

        // Language
        if let Some(lang) = navigator.language() {
            components.push(lang);
        }

        // Platform
        if let Ok(platform) = navigator.platform() {
            components.push(platform);
        }

        // Hash all components
        let combined = components.join("|");
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let hash = hasher.finalize();

        Ok(hex::encode(hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generation() {
        let identity = WasmNodeIdentity::generate("test-site").unwrap();
        assert!(identity.node_id().starts_with("node-"));
        assert_eq!(identity.site_id(), "test-site");
    }

    #[test]
    fn test_sign_verify() {
        let identity = WasmNodeIdentity::generate("test-site").unwrap();
        let message = b"Hello, EdgeNet!";

        let signature = identity.sign(message);
        assert_eq!(signature.len(), 64);

        let is_valid = identity.verify(message, &signature);
        assert!(is_valid);

        // Tampered message should fail
        let is_valid = identity.verify(b"Tampered", &signature);
        assert!(!is_valid);
    }

    // Encrypted export/import tests require WASM environment for JsValue
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_export_import_encrypted() {
        let identity1 = WasmNodeIdentity::generate("test-site").unwrap();
        let password = "secure_password_123";

        // Export with encryption
        let encrypted = identity1.export_secret_key(password).unwrap();

        // Import with decryption
        let identity2 = WasmNodeIdentity::import_secret_key(&encrypted, password, "test-site").unwrap();

        assert_eq!(identity1.node_id(), identity2.node_id());
        assert_eq!(identity1.public_key_hex(), identity2.public_key_hex());
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_export_wrong_password_fails() {
        let identity = WasmNodeIdentity::generate("test-site").unwrap();
        let encrypted = identity.export_secret_key("correct_password").unwrap();

        // Wrong password should fail
        let result = WasmNodeIdentity::import_secret_key(&encrypted, "wrong_password", "test-site");
        assert!(result.is_err());
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_export_short_password_fails() {
        let identity = WasmNodeIdentity::generate("test-site").unwrap();
        // Password too short (< 8 chars)
        let result = identity.export_secret_key("short");
        assert!(result.is_err());
    }
}

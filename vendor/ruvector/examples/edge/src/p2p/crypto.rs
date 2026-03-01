//! Cryptographic Primitives - AES-256-GCM + Canonical Serialization
//!
//! Security principles:
//! - AES-256-GCM authenticated encryption
//! - Canonical JSON with sorted keys (not relying on insertion order)
//! - Proper IV/nonce handling

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use rand::RngCore;

/// Encrypted payload with IV and auth tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub ciphertext: Vec<u8>,
    pub iv: [u8; 12],
    pub tag: [u8; 16],
}

/// Crypto primitives for P2P communication
pub struct CryptoV2;

impl CryptoV2 {
    /// Encrypt data with AES-256-GCM
    pub fn encrypt(data: &[u8], key: &[u8; 32]) -> Result<EncryptedPayload, String> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("Invalid key: {}", e))?;

        // Generate random IV
        let mut iv = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut iv);
        let nonce = Nonce::from_slice(&iv);

        // Encrypt (GCM includes auth tag in output)
        let ciphertext_with_tag = cipher.encrypt(nonce, data)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        // Split ciphertext and tag (tag is last 16 bytes)
        let tag_start = ciphertext_with_tag.len() - 16;
        let ciphertext = ciphertext_with_tag[..tag_start].to_vec();
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&ciphertext_with_tag[tag_start..]);

        Ok(EncryptedPayload { ciphertext, iv, tag })
    }

    /// Decrypt data with AES-256-GCM
    pub fn decrypt(encrypted: &EncryptedPayload, key: &[u8; 32]) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("Invalid key: {}", e))?;

        let nonce = Nonce::from_slice(&encrypted.iv);

        // Reconstruct ciphertext with tag
        let mut ciphertext_with_tag = encrypted.ciphertext.clone();
        ciphertext_with_tag.extend_from_slice(&encrypted.tag);

        cipher.decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|_| "Decryption failed: authentication failed".to_string())
    }

    /// SHA-256 hash
    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// SHA-256 hash as hex string
    pub fn hash_hex(data: &[u8]) -> String {
        hex::encode(Self::hash(data))
    }

    /// Generate a simplified CID (local-only, not real IPFS)
    /// For real IPFS, use multiformats library
    pub fn generate_local_cid(data: &[u8]) -> String {
        let hash = Self::hash_hex(data);
        format!("local:{}", &hash[..32])
    }
}

/// Canonical JSON serialization with sorted keys
/// This is critical for signature verification
pub struct CanonicalJson;

impl CanonicalJson {
    /// Serialize value to canonical JSON (sorted keys, no spaces)
    pub fn stringify(value: &Value) -> String {
        Self::stringify_value(value)
    }

    /// Parse and re-serialize to canonical form
    pub fn canonicalize(json_str: &str) -> Result<String, serde_json::Error> {
        let value: Value = serde_json::from_str(json_str)?;
        Ok(Self::stringify(&value))
    }

    /// Serialize any struct to canonical JSON
    pub fn serialize<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        Ok(Self::stringify(&json_value))
    }

    fn stringify_value(value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => Self::stringify_string(s),
            Value::Array(arr) => Self::stringify_array(arr),
            Value::Object(obj) => Self::stringify_object(obj),
        }
    }

    fn stringify_string(s: &str) -> String {
        // Proper JSON string escaping
        let mut result = String::with_capacity(s.len() + 2);
        result.push('"');
        for c in s.chars() {
            match c {
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                c if c.is_control() => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => result.push(c),
            }
        }
        result.push('"');
        result
    }

    fn stringify_array(arr: &[Value]) -> String {
        let items: Vec<String> = arr.iter().map(Self::stringify_value).collect();
        format!("[{}]", items.join(","))
    }

    fn stringify_object(obj: &serde_json::Map<String, Value>) -> String {
        // Sort keys alphabetically for deterministic output
        let mut keys: Vec<&String> = obj.keys().collect();
        keys.sort();

        let pairs: Vec<String> = keys.iter()
            .filter_map(|k| obj.get(*k).map(|v| {
                format!("{}:{}", Self::stringify_string(k), Self::stringify_value(v))
            }))
            .collect();

        format!("{{{}}}", pairs.join(","))
    }
}

/// Compute hash of canonical JSON representation
pub fn canonical_hash<T: Serialize>(value: &T) -> Result<[u8; 32], serde_json::Error> {
    let canonical = CanonicalJson::serialize(value)?;
    Ok(CryptoV2::hash(canonical.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [1u8; 32];
        let data = b"Hello, World!";

        let encrypted = CryptoV2::encrypt(data, &key).unwrap();
        let decrypted = CryptoV2::decrypt(&encrypted, &key).unwrap();

        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let data = b"Secret data";

        let encrypted = CryptoV2::encrypt(data, &key1).unwrap();
        let result = CryptoV2::decrypt(&encrypted, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_canonical_json_sorted_keys() {
        let json = serde_json::json!({
            "z": 1,
            "a": 2,
            "m": 3
        });

        let canonical = CanonicalJson::stringify(&json);
        assert_eq!(canonical, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn test_canonical_json_nested() {
        let json = serde_json::json!({
            "b": {
                "z": 1,
                "a": 2
            },
            "a": [3, 2, 1]
        });

        let canonical = CanonicalJson::stringify(&json);
        assert_eq!(canonical, r#"{"a":[3,2,1],"b":{"a":2,"z":1}}"#);
    }

    #[test]
    fn test_canonical_json_escaping() {
        let json = serde_json::json!({
            "text": "hello\nworld"
        });

        let canonical = CanonicalJson::stringify(&json);
        assert_eq!(canonical, r#"{"text":"hello\nworld"}"#);
    }

    #[test]
    fn test_canonical_hash_deterministic() {
        #[derive(Serialize)]
        struct Data {
            z: u32,
            a: u32,
        }

        let data1 = Data { z: 1, a: 2 };
        let data2 = Data { z: 1, a: 2 };

        let hash1 = canonical_hash(&data1).unwrap();
        let hash2 = canonical_hash(&data2).unwrap();

        assert_eq!(hash1, hash2);
    }
}

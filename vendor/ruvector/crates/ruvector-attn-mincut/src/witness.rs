use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single witness entry for determinism verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEntry {
    pub q_hash: String,
    pub k_hash: String,
    pub keep_mask: Vec<bool>,
    pub cut_cost: f32,
    pub lambda: f32,
    pub tau: usize,
    pub eps: f32,
    pub timestamp: u64,
}

/// Serialize a witness entry to a single JSONL line.
pub fn witness_log(entry: &WitnessEntry) -> String {
    serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string())
}

/// SHA-256 hash of a float tensor (little-endian bytes), returned as hex.
pub fn hash_tensor(data: &[f32]) -> String {
    let mut h = Sha256::new();
    for &v in data {
        h.update(v.to_le_bytes());
    }
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_deterministic() {
        let d = vec![1.0f32, 2.0, 3.0];
        assert_eq!(hash_tensor(&d), hash_tensor(&d));
        assert_eq!(hash_tensor(&d).len(), 64);
    }

    #[test]
    fn test_hash_differs() {
        assert_ne!(hash_tensor(&[1.0, 2.0]), hash_tensor(&[1.0, 3.0]));
    }

    #[test]
    fn test_witness_roundtrip() {
        let e = WitnessEntry {
            q_hash: "a".into(),
            k_hash: "b".into(),
            keep_mask: vec![true, false],
            cut_cost: 1.5,
            lambda: 0.5,
            tau: 2,
            eps: 0.01,
            timestamp: 1000,
        };
        let json = witness_log(&e);
        let r: WitnessEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(r.q_hash, "a");
        assert!((r.cut_cost - 1.5).abs() < f32::EPSILON);
    }
}

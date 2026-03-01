//! Configuration types for all OSpipe subsystems.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level OSpipe configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsPipeConfig {
    /// Directory for persistent data storage.
    pub data_dir: PathBuf,
    /// Capture subsystem configuration.
    pub capture: CaptureConfig,
    /// Storage subsystem configuration.
    pub storage: StorageConfig,
    /// Search subsystem configuration.
    pub search: SearchConfig,
    /// Safety gate configuration.
    pub safety: SafetyConfig,
}

/// Configuration for the capture subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// Frames per second for screen capture. Default: 1.0
    pub fps: f32,
    /// Duration of audio chunks in seconds. Default: 30
    pub audio_chunk_secs: u32,
    /// Application names to exclude from capture.
    pub excluded_apps: Vec<String>,
    /// Whether to skip windows marked as private/incognito.
    pub skip_private_windows: bool,
}

/// Configuration for the vector storage subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Dimensionality of embedding vectors. Default: 384
    pub embedding_dim: usize,
    /// HNSW M parameter (max connections per layer). Default: 32
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter. Default: 200
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter. Default: 100
    pub hnsw_ef_search: usize,
    /// Cosine similarity threshold for deduplication. Default: 0.95
    pub dedup_threshold: f32,
    /// Quantization tiers for aging data.
    pub quantization_tiers: Vec<QuantizationTier>,
}

/// A quantization tier that defines how vectors are compressed based on age.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationTier {
    /// Age in hours after which this quantization is applied.
    pub age_hours: u64,
    /// The quantization method to use.
    pub method: QuantizationMethod,
}

/// Supported vector quantization methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationMethod {
    /// No quantization (full precision f32).
    None,
    /// Scalar quantization (int8).
    Scalar,
    /// Product quantization.
    Product,
    /// Binary quantization (1-bit per dimension).
    Binary,
}

/// Configuration for the search subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Default number of results to return. Default: 10
    pub default_k: usize,
    /// Weight for semantic vs keyword search in hybrid mode. Default: 0.7
    /// 1.0 = pure semantic, 0.0 = pure keyword.
    pub hybrid_weight: f32,
    /// MMR lambda for diversity vs relevance tradeoff. Default: 0.5
    pub mmr_lambda: f32,
    /// Whether to enable result reranking.
    pub rerank_enabled: bool,
}

/// Configuration for the safety gate subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Enable PII detection (names, emails, phone numbers).
    pub pii_detection: bool,
    /// Enable credit card number redaction.
    pub credit_card_redaction: bool,
    /// Enable SSN redaction.
    pub ssn_redaction: bool,
    /// Custom regex-like patterns to redact (simple substring matching).
    pub custom_patterns: Vec<String>,
}

impl Default for OsPipeConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("~/.ospipe"),
            capture: CaptureConfig::default(),
            storage: StorageConfig::default(),
            search: SearchConfig::default(),
            safety: SafetyConfig::default(),
        }
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            fps: 1.0,
            audio_chunk_secs: 30,
            excluded_apps: vec!["1Password".to_string(), "Keychain Access".to_string()],
            skip_private_windows: true,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 384,
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
            dedup_threshold: 0.95,
            quantization_tiers: vec![
                QuantizationTier {
                    age_hours: 0,
                    method: QuantizationMethod::None,
                },
                QuantizationTier {
                    age_hours: 24,
                    method: QuantizationMethod::Scalar,
                },
                QuantizationTier {
                    age_hours: 168, // 1 week
                    method: QuantizationMethod::Product,
                },
                QuantizationTier {
                    age_hours: 720, // 30 days
                    method: QuantizationMethod::Binary,
                },
            ],
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_k: 10,
            hybrid_weight: 0.7,
            mmr_lambda: 0.5,
            rerank_enabled: false,
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            pii_detection: true,
            credit_card_redaction: true,
            ssn_redaction: true,
            custom_patterns: Vec::new(),
        }
    }
}

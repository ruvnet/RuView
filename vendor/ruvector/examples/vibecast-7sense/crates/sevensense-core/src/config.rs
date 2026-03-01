//! # Configuration Module
//!
//! Configuration management for the 7sense platform.
//!
//! This module provides:
//! - Typed configuration structures
//! - Environment variable loading
//! - Configuration file parsing
//! - Default values and validation

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::domain::errors::ConfigurationError;

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Application name.
    pub name: String,
    /// Application environment (development, staging, production).
    pub environment: Environment,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Audio processing configuration.
    pub audio: AudioConfig,
    /// Embedding configuration.
    pub embedding: EmbeddingConfig,
    /// Vector database configuration.
    pub vector_db: VectorDbConfig,
    /// API server configuration.
    pub api: ApiConfig,
    /// Storage configuration.
    pub storage: StorageConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "sevensense".to_string(),
            environment: Environment::default(),
            logging: LoggingConfig::default(),
            audio: AudioConfig::default(),
            embedding: EmbeddingConfig::default(),
            vector_db: VectorDbConfig::default(),
            api: ApiConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from environment and files.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration cannot be loaded or is invalid.
    pub fn load() -> Result<Self, ConfigurationError> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let mut builder = config::Config::builder();

        // Add default values
        builder = builder.add_source(config::Config::try_from(&Self::default()).map_err(|e| {
            ConfigurationError::ParseError {
                reason: e.to_string(),
            }
        })?);

        // Try to load from config file
        let config_path = std::env::var("SEVENSENSE_CONFIG")
            .unwrap_or_else(|_| "config/default.toml".to_string());

        if std::path::Path::new(&config_path).exists() {
            builder = builder.add_source(config::File::with_name(&config_path));
        }

        // Override with environment variables (SEVENSENSE_ prefix)
        builder = builder.add_source(
            config::Environment::with_prefix("SEVENSENSE")
                .separator("__")
                .try_parsing(true),
        );

        let config = builder
            .build()
            .map_err(|e| ConfigurationError::ParseError {
                reason: e.to_string(),
            })?;

        config
            .try_deserialize()
            .map_err(|e| ConfigurationError::ParseError {
                reason: e.to_string(),
            })
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is invalid.
    pub fn validate(&self) -> Result<(), ConfigurationError> {
        // Validate audio settings
        if self.audio.min_segment_duration_ms >= self.audio.max_segment_duration_ms {
            return Err(ConfigurationError::Invalid {
                key: "audio.min_segment_duration_ms".to_string(),
                reason: "must be less than max_segment_duration_ms".to_string(),
            });
        }

        // Validate embedding dimensions
        if self.embedding.dimensions == 0 {
            return Err(ConfigurationError::Invalid {
                key: "embedding.dimensions".to_string(),
                reason: "must be greater than 0".to_string(),
            });
        }

        // Validate API settings
        if self.api.port == 0 {
            return Err(ConfigurationError::Invalid {
                key: "api.port".to_string(),
                reason: "must be a valid port number".to_string(),
            });
        }

        Ok(())
    }

    /// Returns whether this is a production environment.
    #[must_use]
    pub fn is_production(&self) -> bool {
        matches!(self.environment, Environment::Production)
    }

    /// Returns whether this is a development environment.
    #[must_use]
    pub fn is_development(&self) -> bool {
        matches!(self.environment, Environment::Development)
    }
}

/// Application environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    /// Development environment.
    #[default]
    Development,
    /// Staging environment.
    Staging,
    /// Production environment.
    Production,
}

impl Environment {
    /// Returns the environment name as a string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Environment {
    type Err = ConfigurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(ConfigurationError::Invalid {
                key: "environment".to_string(),
                reason: format!("unknown environment: {s}"),
            }),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).
    pub level: String,
    /// Output format (json, pretty).
    pub format: LogFormat,
    /// Whether to include source code location.
    pub include_location: bool,
    /// Whether to include span events.
    pub include_spans: bool,
    /// OpenTelemetry configuration.
    pub opentelemetry: Option<OpenTelemetryConfig>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::default(),
            include_location: false,
            include_spans: true,
            opentelemetry: None,
        }
    }
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// JSON format (for structured logging).
    #[default]
    Json,
    /// Pretty format (human-readable, for development).
    Pretty,
    /// Compact format (single line, minimal).
    Compact,
}

/// OpenTelemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryConfig {
    /// OTLP endpoint URL.
    pub endpoint: String,
    /// Service name for traces.
    pub service_name: String,
    /// Sampling ratio (0.0 to 1.0).
    pub sampling_ratio: f64,
}

/// Audio processing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Supported input sample rates in Hz.
    pub supported_sample_rates: Vec<u32>,
    /// Target sample rate for processing in Hz.
    pub target_sample_rate: u32,
    /// Maximum file size in bytes.
    pub max_file_size_bytes: u64,
    /// Minimum segment duration in milliseconds.
    pub min_segment_duration_ms: u64,
    /// Maximum segment duration in milliseconds.
    pub max_segment_duration_ms: u64,
    /// Default segment overlap ratio (0.0 to 1.0).
    pub segment_overlap_ratio: f32,
    /// Energy threshold for segment detection.
    pub energy_threshold: f32,
    /// Frequency range for analysis (Hz).
    pub frequency_range: (f32, f32),
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            supported_sample_rates: vec![16000, 22050, 44100, 48000, 96000],
            target_sample_rate: 48000,
            max_file_size_bytes: 500 * 1024 * 1024, // 500 MB
            min_segment_duration_ms: 100,
            max_segment_duration_ms: 30000, // 30 seconds
            segment_overlap_ratio: 0.25,
            energy_threshold: 0.01,
            frequency_range: (50.0, 15000.0),
        }
    }
}

/// Embedding model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Model name or path.
    pub model_name: String,
    /// Model version.
    pub model_version: String,
    /// Embedding dimensions.
    pub dimensions: u32,
    /// Batch size for embedding generation.
    pub batch_size: usize,
    /// Whether to use GPU acceleration.
    pub use_gpu: bool,
    /// Model inference timeout.
    #[serde(with = "humantime_serde")]
    pub inference_timeout: Duration,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_name: "birdnet-v2.4".to_string(),
            model_version: "2.4.0".to_string(),
            dimensions: 1024,
            batch_size: 32,
            use_gpu: false,
            inference_timeout: Duration::from_secs(30),
        }
    }
}

/// Vector database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VectorDbConfig {
    /// Vector database URL.
    pub url: String,
    /// API key (if required).
    pub api_key: Option<String>,
    /// Collection name for embeddings.
    pub collection_name: String,
    /// Number of vectors to return in searches.
    pub default_limit: u32,
    /// HNSW index parameters.
    pub hnsw: HnswConfig,
    /// Connection timeout.
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,
}

impl Default for VectorDbConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:6334".to_string(),
            api_key: None,
            collection_name: "sevensense_embeddings".to_string(),
            default_limit: 10,
            hnsw: HnswConfig::default(),
            connection_timeout: Duration::from_secs(10),
        }
    }
}

/// HNSW index configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HnswConfig {
    /// Number of edges per node.
    pub m: u32,
    /// Size of the dynamic candidate list during construction.
    pub ef_construct: u32,
    /// Size of the dynamic candidate list during search.
    pub ef_search: u32,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construct: 100,
            ef_search: 64,
        }
    }
}

/// API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    /// Host to bind to.
    pub host: String,
    /// Port to listen on.
    pub port: u16,
    /// Request body size limit in bytes.
    pub body_limit_bytes: usize,
    /// Request timeout.
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
    /// CORS allowed origins.
    pub cors_origins: Vec<String>,
    /// Rate limiting configuration.
    pub rate_limit: RateLimitConfig,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            body_limit_bytes: 100 * 1024 * 1024, // 100 MB
            request_timeout: Duration::from_secs(300),
            cors_origins: vec!["*".to_string()],
            rate_limit: RateLimitConfig::default(),
        }
    }
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled.
    pub enabled: bool,
    /// Maximum requests per second.
    pub requests_per_second: u32,
    /// Burst size.
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_size: 200,
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage backend type.
    pub backend: StorageBackend,
    /// Local storage path (for filesystem backend).
    pub local_path: PathBuf,
    /// S3 configuration (for S3 backend).
    pub s3: Option<S3Config>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::default(),
            local_path: PathBuf::from("./data/storage"),
            s3: None,
        }
    }
}

/// Storage backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// Local filesystem storage.
    #[default]
    Filesystem,
    /// Amazon S3 or S3-compatible storage.
    S3,
}

/// S3 storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region.
    pub region: String,
    /// S3 endpoint URL (for S3-compatible services).
    pub endpoint: Option<String>,
    /// Path prefix within the bucket.
    pub prefix: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.name, "sevensense");
        assert!(config.is_development());
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        // Invalid: min >= max
        config.audio.min_segment_duration_ms = 5000;
        config.audio.max_segment_duration_ms = 1000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_parsing() {
        assert_eq!(
            "development".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "prod".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert!("invalid".parse::<Environment>().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.name, parsed.name);
    }
}

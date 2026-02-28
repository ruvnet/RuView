//! Configuration system for Ruvector-Scipix
//!
//! Comprehensive configuration with TOML support, environment overrides, and validation.

use crate::error::{Result, ScipixError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// OCR processing configuration
    pub ocr: OcrConfig,

    /// Model configuration
    pub model: ModelConfig,

    /// Preprocessing configuration
    pub preprocess: PreprocessConfig,

    /// Output format configuration
    pub output: OutputConfig,

    /// Performance tuning
    pub performance: PerformanceConfig,

    /// Cache configuration
    pub cache: CacheConfig,
}

/// OCR engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Confidence threshold (0.0-1.0)
    pub confidence_threshold: f32,

    /// Maximum processing time in seconds
    pub timeout: u64,

    /// Enable GPU acceleration
    pub use_gpu: bool,

    /// Language codes (e.g., ["en", "es"])
    pub languages: Vec<String>,

    /// Enable equation detection
    pub detect_equations: bool,

    /// Enable table detection
    pub detect_tables: bool,
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Path to OCR model
    pub model_path: String,

    /// Model version
    pub version: String,

    /// Batch size for processing
    pub batch_size: usize,

    /// Model precision (fp16, fp32, int8)
    pub precision: String,

    /// Enable quantization
    pub quantize: bool,
}

/// Image preprocessing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessConfig {
    /// Enable auto-rotation
    pub auto_rotate: bool,

    /// Enable denoising
    pub denoise: bool,

    /// Enable contrast enhancement
    pub enhance_contrast: bool,

    /// Enable binarization
    pub binarize: bool,

    /// Target DPI for scaling
    pub target_dpi: u32,

    /// Maximum image dimension
    pub max_dimension: u32,
}

/// Output format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output formats (latex, mathml, asciimath)
    pub formats: Vec<String>,

    /// Include confidence scores
    pub include_confidence: bool,

    /// Include bounding boxes
    pub include_bbox: bool,

    /// Pretty print JSON
    pub pretty_print: bool,

    /// Include metadata
    pub include_metadata: bool,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    pub num_threads: usize,

    /// Enable parallel processing
    pub parallel: bool,

    /// Memory limit in MB
    pub memory_limit: usize,

    /// Enable profiling
    pub profile: bool,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,

    /// Cache capacity (number of entries)
    pub capacity: usize,

    /// Similarity threshold for cache hits (0.0-1.0)
    pub similarity_threshold: f32,

    /// Cache TTL in seconds
    pub ttl: u64,

    /// Vector dimension for embeddings
    pub vector_dimension: usize,

    /// Enable persistent cache
    pub persistent: bool,

    /// Cache directory path
    pub cache_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ocr: OcrConfig {
                confidence_threshold: 0.7,
                timeout: 30,
                use_gpu: false,
                languages: vec!["en".to_string()],
                detect_equations: true,
                detect_tables: true,
            },
            model: ModelConfig {
                model_path: "models/scipix-ocr".to_string(),
                version: "1.0.0".to_string(),
                batch_size: 1,
                precision: "fp32".to_string(),
                quantize: false,
            },
            preprocess: PreprocessConfig {
                auto_rotate: true,
                denoise: true,
                enhance_contrast: true,
                binarize: false,
                target_dpi: 300,
                max_dimension: 4096,
            },
            output: OutputConfig {
                formats: vec!["latex".to_string()],
                include_confidence: true,
                include_bbox: false,
                pretty_print: true,
                include_metadata: false,
            },
            performance: PerformanceConfig {
                num_threads: num_cpus::get(),
                parallel: true,
                memory_limit: 2048,
                profile: false,
            },
            cache: CacheConfig {
                enabled: true,
                capacity: 1000,
                similarity_threshold: 0.95,
                ttl: 3600,
                vector_dimension: 512,
                persistent: false,
                cache_dir: ".cache/scipix".to_string(),
            },
        }
    }
}

impl Config {
    /// Load configuration from TOML file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to TOML configuration file
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use ruvector_scipix::Config;
    ///
    /// let config = Config::from_file("scipix.toml").unwrap();
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to TOML file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to save TOML configuration
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load configuration from environment variables
    ///
    /// Environment variables should be prefixed with `MATHPIX_`
    /// and use double underscores for nested fields.
    ///
    /// # Examples
    ///
    /// ```bash
    /// export MATHPIX_OCR__CONFIDENCE_THRESHOLD=0.8
    /// export MATHPIX_MODEL__BATCH_SIZE=4
    /// ```
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();
        config.apply_env_overrides()?;
        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) -> Result<()> {
        // OCR overrides
        if let Ok(val) = std::env::var("MATHPIX_OCR__CONFIDENCE_THRESHOLD") {
            self.ocr.confidence_threshold = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid confidence_threshold".to_string()))?;
        }
        if let Ok(val) = std::env::var("MATHPIX_OCR__TIMEOUT") {
            self.ocr.timeout = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid timeout".to_string()))?;
        }
        if let Ok(val) = std::env::var("MATHPIX_OCR__USE_GPU") {
            self.ocr.use_gpu = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid use_gpu".to_string()))?;
        }

        // Model overrides
        if let Ok(val) = std::env::var("MATHPIX_MODEL__PATH") {
            self.model.model_path = val;
        }
        if let Ok(val) = std::env::var("MATHPIX_MODEL__BATCH_SIZE") {
            self.model.batch_size = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid batch_size".to_string()))?;
        }

        // Cache overrides
        if let Ok(val) = std::env::var("MATHPIX_CACHE__ENABLED") {
            self.cache.enabled = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid cache enabled".to_string()))?;
        }
        if let Ok(val) = std::env::var("MATHPIX_CACHE__CAPACITY") {
            self.cache.capacity = val
                .parse()
                .map_err(|_| ScipixError::Config("Invalid cache capacity".to_string()))?;
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate confidence threshold
        if self.ocr.confidence_threshold < 0.0 || self.ocr.confidence_threshold > 1.0 {
            return Err(ScipixError::Config(
                "confidence_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Validate similarity threshold
        if self.cache.similarity_threshold < 0.0 || self.cache.similarity_threshold > 1.0 {
            return Err(ScipixError::Config(
                "similarity_threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Validate batch size
        if self.model.batch_size == 0 {
            return Err(ScipixError::Config(
                "batch_size must be greater than 0".to_string(),
            ));
        }

        // Validate precision
        let valid_precisions = ["fp16", "fp32", "int8"];
        if !valid_precisions.contains(&self.model.precision.as_str()) {
            return Err(ScipixError::Config(format!(
                "precision must be one of: {:?}",
                valid_precisions
            )));
        }

        // Validate output formats
        let valid_formats = ["latex", "mathml", "asciimath"];
        for format in &self.output.formats {
            if !valid_formats.contains(&format.as_str()) {
                return Err(ScipixError::Config(format!(
                    "Invalid output format: {}. Must be one of: {:?}",
                    format, valid_formats
                )));
            }
        }

        Ok(())
    }

    /// Create high-accuracy preset configuration
    pub fn high_accuracy() -> Self {
        let mut config = Self::default();
        config.ocr.confidence_threshold = 0.9;
        config.model.precision = "fp32".to_string();
        config.model.quantize = false;
        config.preprocess.denoise = true;
        config.preprocess.enhance_contrast = true;
        config.cache.similarity_threshold = 0.98;
        config
    }

    /// Create high-speed preset configuration
    pub fn high_speed() -> Self {
        let mut config = Self::default();
        config.ocr.confidence_threshold = 0.6;
        config.model.precision = "fp16".to_string();
        config.model.quantize = true;
        config.model.batch_size = 4;
        config.preprocess.denoise = false;
        config.preprocess.enhance_contrast = false;
        config.performance.parallel = true;
        config.cache.similarity_threshold = 0.85;
        config
    }

    /// Create minimal configuration
    pub fn minimal() -> Self {
        let mut config = Self::default();
        config.cache.enabled = false;
        config.preprocess.denoise = false;
        config.preprocess.enhance_contrast = false;
        config.performance.parallel = false;
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.ocr.confidence_threshold, 0.7);
        assert!(config.cache.enabled);
    }

    #[test]
    fn test_high_accuracy_config() {
        let config = Config::high_accuracy();
        assert!(config.validate().is_ok());
        assert_eq!(config.ocr.confidence_threshold, 0.9);
        assert_eq!(config.cache.similarity_threshold, 0.98);
    }

    #[test]
    fn test_high_speed_config() {
        let config = Config::high_speed();
        assert!(config.validate().is_ok());
        assert_eq!(config.model.precision, "fp16");
        assert!(config.model.quantize);
    }

    #[test]
    fn test_minimal_config() {
        let config = Config::minimal();
        assert!(config.validate().is_ok());
        assert!(!config.cache.enabled);
    }

    #[test]
    fn test_invalid_confidence_threshold() {
        let mut config = Config::default();
        config.ocr.confidence_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_batch_size() {
        let mut config = Config::default();
        config.model.batch_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_precision() {
        let mut config = Config::default();
        config.model.precision = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_output_format() {
        let mut config = Config::default();
        config.output.formats = vec!["invalid".to_string()];
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_toml_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            config.ocr.confidence_threshold,
            deserialized.ocr.confidence_threshold
        );
    }
}

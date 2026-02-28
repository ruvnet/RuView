//! # Ruvector-Scipix
//!
//! A high-performance Rust implementation of Scipix OCR for mathematical expressions and equations.
//! Built on top of ruvector-core for efficient vector-based caching and similarity search.
//!
//! ## Features
//!
//! - **Mathematical OCR**: Extract LaTeX from images of equations
//! - **Vector Caching**: Intelligent caching using image embeddings
//! - **Multiple Formats**: Support for LaTeX, MathML, AsciiMath
//! - **High Performance**: Parallel processing and efficient caching
//! - **Configurable**: Extensive configuration options via TOML or API
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ruvector_scipix::{Config, OcrEngine, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Load configuration
//!     let config = Config::from_file("scipix.toml")?;
//!
//!     // Create OCR engine
//!     let engine = OcrEngine::new(config).await?;
//!
//!     // Process image
//!     let result = engine.process_image("equation.png").await?;
//!     println!("LaTeX: {}", result.latex);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//!
//! - **config**: Configuration management with TOML support
//! - **error**: Comprehensive error types with context
//! - **math**: LaTeX and mathematical format handling
//! - **ocr**: Core OCR processing engine
//! - **output**: Output formatting and serialization
//! - **preprocess**: Image preprocessing pipeline
//! - **cache**: Vector-based intelligent caching

// Module declarations
pub mod api;
pub mod cli;
pub mod config;
pub mod error;

#[cfg(feature = "cache")]
pub mod cache;

#[cfg(feature = "ocr")]
pub mod ocr;

#[cfg(feature = "math")]
pub mod math;

#[cfg(feature = "preprocess")]
pub mod preprocess;

// Output module is always available
pub mod output;

// Performance optimizations
#[cfg(feature = "optimize")]
pub mod optimize;

// WebAssembly bindings
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub mod wasm;

// Public re-exports
pub use api::{state::AppState, ApiServer};
pub use cli::{Cli, Commands};
pub use config::{
    CacheConfig, Config, ModelConfig, OcrConfig, OutputConfig, PerformanceConfig, PreprocessConfig,
};
pub use error::{Result, ScipixError};

#[cfg(feature = "cache")]
pub use cache::CacheManager;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default configuration preset
pub fn default_config() -> Config {
    Config::default()
}

/// High-accuracy configuration preset
pub fn high_accuracy_config() -> Config {
    Config::high_accuracy()
}

/// High-speed configuration preset
pub fn high_speed_config() -> Config {
    Config::high_speed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_default_config() {
        let config = default_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_high_accuracy_config() {
        let config = high_accuracy_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_high_speed_config() {
        let config = high_speed_config();
        assert!(config.validate().is_ok());
    }
}

//! Configuration management for Ruvector CLI

use anyhow::{Context, Result};
use ruvector_core::types::{DbOptions, DistanceMetric, HnswConfig, QuantizationConfig};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Ruvector CLI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Database options
    #[serde(default)]
    pub database: DatabaseConfig,

    /// CLI options
    #[serde(default)]
    pub cli: CliConfig,

    /// MCP server options
    #[serde(default)]
    pub mcp: McpConfig,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Default storage path
    #[serde(default = "default_storage_path")]
    pub storage_path: String,

    /// Default dimensions
    #[serde(default = "default_dimensions")]
    pub dimensions: usize,

    /// Distance metric
    #[serde(default = "default_distance_metric")]
    pub distance_metric: DistanceMetric,

    /// HNSW configuration
    #[serde(default)]
    pub hnsw: Option<HnswConfig>,

    /// Quantization configuration
    #[serde(default)]
    pub quantization: Option<QuantizationConfig>,
}

/// CLI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Show progress bars
    #[serde(default = "default_true")]
    pub progress: bool,

    /// Use colors in output
    #[serde(default = "default_true")]
    pub colors: bool,

    /// Default batch size for operations
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server host for SSE transport
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port for SSE transport
    #[serde(default = "default_port")]
    pub port: u16,

    /// Enable CORS
    #[serde(default = "default_true")]
    pub cors: bool,

    /// Allowed data directory for MCP file operations (path confinement)
    /// All db_path and backup_path values must resolve within this directory.
    /// Defaults to the current working directory.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

// Default value functions
fn default_storage_path() -> String {
    "./ruvector.db".to_string()
}

fn default_dimensions() -> usize {
    384
}

fn default_distance_metric() -> DistanceMetric {
    DistanceMetric::Cosine
}

fn default_true() -> bool {
    true
}

fn default_batch_size() -> usize {
    1000
}

fn default_data_dir() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            cli: CliConfig::default(),
            mcp: McpConfig::default(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            storage_path: default_storage_path(),
            dimensions: default_dimensions(),
            distance_metric: DistanceMetric::Cosine,
            hnsw: Some(HnswConfig::default()),
            quantization: Some(QuantizationConfig::Scalar),
        }
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            progress: true,
            colors: true,
            batch_size: default_batch_size(),
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            cors: true,
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content =
            std::fs::read_to_string(path.as_ref()).context("Failed to read config file")?;
        let config: Config = toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    /// Load configuration with precedence: CLI args > env vars > config file > defaults
    pub fn load(config_path: Option<PathBuf>) -> Result<Self> {
        let mut config = if let Some(path) = config_path {
            Self::from_file(&path).unwrap_or_default()
        } else {
            // Try default locations
            Self::try_default_locations().unwrap_or_default()
        };

        // Override with environment variables
        config.apply_env_vars()?;

        Ok(config)
    }

    /// Try loading from default locations
    fn try_default_locations() -> Option<Self> {
        let paths = vec![
            "ruvector.toml",
            ".ruvector.toml",
            "~/.config/ruvector/config.toml",
            "/etc/ruvector/config.toml",
        ];

        for path in paths {
            let expanded = shellexpand::tilde(path).to_string();
            if let Ok(config) = Self::from_file(&expanded) {
                return Some(config);
            }
        }

        None
    }

    /// Apply environment variable overrides
    fn apply_env_vars(&mut self) -> Result<()> {
        if let Ok(path) = std::env::var("RUVECTOR_STORAGE_PATH") {
            self.database.storage_path = path;
        }

        if let Ok(dims) = std::env::var("RUVECTOR_DIMENSIONS") {
            self.database.dimensions = dims.parse().context("Invalid RUVECTOR_DIMENSIONS")?;
        }

        if let Ok(metric) = std::env::var("RUVECTOR_DISTANCE_METRIC") {
            self.database.distance_metric = match metric.to_lowercase().as_str() {
                "euclidean" => DistanceMetric::Euclidean,
                "cosine" => DistanceMetric::Cosine,
                "dotproduct" => DistanceMetric::DotProduct,
                "manhattan" => DistanceMetric::Manhattan,
                _ => return Err(anyhow::anyhow!("Invalid distance metric: {}", metric)),
            };
        }

        if let Ok(host) = std::env::var("RUVECTOR_MCP_HOST") {
            self.mcp.host = host;
        }

        if let Ok(port) = std::env::var("RUVECTOR_MCP_PORT") {
            self.mcp.port = port.parse().context("Invalid RUVECTOR_MCP_PORT")?;
        }

        if let Ok(data_dir) = std::env::var("RUVECTOR_MCP_DATA_DIR") {
            self.mcp.data_dir = data_dir;
        }

        Ok(())
    }

    /// Convert to DbOptions
    pub fn to_db_options(&self) -> DbOptions {
        DbOptions {
            dimensions: self.database.dimensions,
            distance_metric: self.database.distance_metric,
            storage_path: self.database.storage_path.clone(),
            hnsw_config: self.database.hnsw.clone(),
            quantization: self.database.quantization.clone(),
        }
    }

    /// Save configuration to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(path, content).context("Failed to write config file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.database.dimensions, 384);
        assert_eq!(config.cli.batch_size, 1000);
        assert_eq!(config.mcp.port, 3000);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.database.dimensions, parsed.database.dimensions);
    }
}

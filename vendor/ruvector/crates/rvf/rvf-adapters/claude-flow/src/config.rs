//! Configuration for the claude-flow memory adapter.

use std::path::PathBuf;

use rvf_runtime::options::DistanceMetric;

/// Configuration for the RVF-backed claude-flow memory store.
#[derive(Clone, Debug)]
pub struct ClaudeFlowConfig {
    /// Directory where RVF data files are stored.
    pub data_dir: PathBuf,
    /// Vector embedding dimension (must match the embeddings used by claude-flow).
    pub dimension: u16,
    /// Distance metric for similarity search.
    pub metric: DistanceMetric,
    /// Whether to record witness entries for audit trails.
    pub enable_witness: bool,
}

impl ClaudeFlowConfig {
    /// Create a new configuration with required parameters.
    pub fn new(data_dir: impl Into<PathBuf>, dimension: u16) -> Self {
        Self {
            data_dir: data_dir.into(),
            dimension,
            metric: DistanceMetric::Cosine,
            enable_witness: true,
        }
    }

    /// Set the distance metric.
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Enable or disable witness audit trails.
    pub fn with_witness(mut self, enable: bool) -> Self {
        self.enable_witness = enable;
        self
    }

    /// Return the path to the main vector store RVF file.
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join("memory.rvf")
    }

    /// Return the path to the witness chain file.
    pub fn witness_path(&self) -> PathBuf {
        self.data_dir.join("witness.bin")
    }

    /// Ensure the data directory exists.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.dimension == 0 {
            return Err(ConfigError::InvalidDimension);
        }
        Ok(())
    }
}

/// Errors specific to adapter configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Dimension must be > 0.
    InvalidDimension,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimension => write!(f, "vector dimension must be > 0"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn config_defaults() {
        let cfg = ClaudeFlowConfig::new("/tmp/test", 384);
        assert_eq!(cfg.dimension, 384);
        assert_eq!(cfg.metric, DistanceMetric::Cosine);
        assert!(cfg.enable_witness);
    }

    #[test]
    fn config_paths() {
        let cfg = ClaudeFlowConfig::new("/data/memory", 128);
        assert_eq!(cfg.store_path(), Path::new("/data/memory/memory.rvf"));
        assert_eq!(cfg.witness_path(), Path::new("/data/memory/witness.bin"));
    }

    #[test]
    fn validate_zero_dimension() {
        let cfg = ClaudeFlowConfig::new("/tmp", 0);
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidDimension));
    }

    #[test]
    fn validate_ok() {
        let cfg = ClaudeFlowConfig::new("/tmp", 64);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn builder_methods() {
        let cfg = ClaudeFlowConfig::new("/tmp", 256)
            .with_metric(DistanceMetric::L2)
            .with_witness(false);
        assert_eq!(cfg.metric, DistanceMetric::L2);
        assert!(!cfg.enable_witness);
    }
}

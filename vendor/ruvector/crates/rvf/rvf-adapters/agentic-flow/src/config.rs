//! Configuration for the agentic-flow swarm adapter.

use std::path::PathBuf;

/// Configuration for the RVF-backed agentic-flow swarm store.
#[derive(Clone, Debug)]
pub struct AgenticFlowConfig {
    /// Directory where RVF data files are stored.
    pub data_dir: PathBuf,
    /// Vector embedding dimension (must match embeddings used by agents).
    pub dimension: u16,
    /// Unique identifier for this agent.
    pub agent_id: String,
    /// Whether to log consensus events in a WITNESS_SEG audit trail.
    pub enable_witness: bool,
    /// Optional swarm group identifier for multi-swarm deployments.
    pub swarm_id: Option<String>,
}

impl AgenticFlowConfig {
    /// Create a new configuration with required parameters.
    ///
    /// Uses sensible defaults: dimension=384, witness enabled, no swarm group.
    pub fn new(data_dir: impl Into<PathBuf>, agent_id: impl Into<String>) -> Self {
        Self {
            data_dir: data_dir.into(),
            dimension: 384,
            agent_id: agent_id.into(),
            enable_witness: true,
            swarm_id: None,
        }
    }

    /// Set the embedding dimension.
    pub fn with_dimension(mut self, dimension: u16) -> Self {
        self.dimension = dimension;
        self
    }

    /// Enable or disable witness audit trails.
    pub fn with_witness(mut self, enable: bool) -> Self {
        self.enable_witness = enable;
        self
    }

    /// Set the swarm group identifier.
    pub fn with_swarm_id(mut self, swarm_id: impl Into<String>) -> Self {
        self.swarm_id = Some(swarm_id.into());
        self
    }

    /// Return the path to the main vector store RVF file.
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join("swarm.rvf")
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
        if self.agent_id.is_empty() {
            return Err(ConfigError::EmptyAgentId);
        }
        Ok(())
    }
}

/// Errors specific to adapter configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Dimension must be > 0.
    InvalidDimension,
    /// Agent ID must not be empty.
    EmptyAgentId,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimension => write!(f, "vector dimension must be > 0"),
            Self::EmptyAgentId => write!(f, "agent_id must not be empty"),
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
        let cfg = AgenticFlowConfig::new("/tmp/test", "agent-1");
        assert_eq!(cfg.dimension, 384);
        assert!(cfg.enable_witness);
        assert!(cfg.swarm_id.is_none());
        assert_eq!(cfg.agent_id, "agent-1");
    }

    #[test]
    fn config_paths() {
        let cfg = AgenticFlowConfig::new("/data/swarm", "a1");
        assert_eq!(cfg.store_path(), Path::new("/data/swarm/swarm.rvf"));
        assert_eq!(cfg.witness_path(), Path::new("/data/swarm/witness.bin"));
    }

    #[test]
    fn validate_zero_dimension() {
        let cfg = AgenticFlowConfig::new("/tmp", "a1").with_dimension(0);
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidDimension));
    }

    #[test]
    fn validate_empty_agent_id() {
        let cfg = AgenticFlowConfig::new("/tmp", "");
        assert_eq!(cfg.validate(), Err(ConfigError::EmptyAgentId));
    }

    #[test]
    fn validate_ok() {
        let cfg = AgenticFlowConfig::new("/tmp", "agent-1").with_dimension(64);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn builder_methods() {
        let cfg = AgenticFlowConfig::new("/tmp", "a1")
            .with_dimension(128)
            .with_witness(false)
            .with_swarm_id("swarm-alpha");
        assert_eq!(cfg.dimension, 128);
        assert!(!cfg.enable_witness);
        assert_eq!(cfg.swarm_id.as_deref(), Some("swarm-alpha"));
    }
}

//! Configuration for the SONA adapter.

use std::path::PathBuf;

/// Configuration for the RVF-backed SONA stores.
#[derive(Clone, Debug)]
pub struct SonaConfig {
    /// Directory where RVF data files are stored.
    pub data_dir: PathBuf,
    /// Vector embedding dimension (must match SONA's embedding size).
    pub dimension: u16,
    /// Maximum number of experiences in the replay buffer.
    pub replay_capacity: usize,
    /// Number of recent trajectory steps to retain in the window.
    pub trajectory_window: usize,
}

impl SonaConfig {
    /// Create a new configuration with required parameters and sensible defaults.
    pub fn new(data_dir: impl Into<PathBuf>, dimension: u16) -> Self {
        Self {
            data_dir: data_dir.into(),
            dimension,
            replay_capacity: 10_000,
            trajectory_window: 100,
        }
    }

    /// Set the replay buffer capacity.
    pub fn with_replay_capacity(mut self, capacity: usize) -> Self {
        self.replay_capacity = capacity;
        self
    }

    /// Set the trajectory window size.
    pub fn with_trajectory_window(mut self, window: usize) -> Self {
        self.trajectory_window = window;
        self
    }

    /// Return the path to the shared RVF store file.
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join("sona.rvf")
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
        if self.replay_capacity == 0 {
            return Err(ConfigError::InvalidReplayCapacity);
        }
        if self.trajectory_window == 0 {
            return Err(ConfigError::InvalidTrajectoryWindow);
        }
        Ok(())
    }
}

/// Errors specific to adapter configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Dimension must be > 0.
    InvalidDimension,
    /// Replay capacity must be > 0.
    InvalidReplayCapacity,
    /// Trajectory window must be > 0.
    InvalidTrajectoryWindow,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimension => write!(f, "vector dimension must be > 0"),
            Self::InvalidReplayCapacity => write!(f, "replay capacity must be > 0"),
            Self::InvalidTrajectoryWindow => write!(f, "trajectory window must be > 0"),
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
        let cfg = SonaConfig::new("/tmp/test", 256);
        assert_eq!(cfg.dimension, 256);
        assert_eq!(cfg.replay_capacity, 10_000);
        assert_eq!(cfg.trajectory_window, 100);
    }

    #[test]
    fn config_store_path() {
        let cfg = SonaConfig::new("/data/sona", 128);
        assert_eq!(cfg.store_path(), Path::new("/data/sona/sona.rvf"));
    }

    #[test]
    fn validate_zero_dimension() {
        let cfg = SonaConfig::new("/tmp", 0);
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidDimension));
    }

    #[test]
    fn validate_zero_replay_capacity() {
        let mut cfg = SonaConfig::new("/tmp", 64);
        cfg.replay_capacity = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidReplayCapacity));
    }

    #[test]
    fn validate_zero_trajectory_window() {
        let mut cfg = SonaConfig::new("/tmp", 64);
        cfg.trajectory_window = 0;
        assert_eq!(cfg.validate(), Err(ConfigError::InvalidTrajectoryWindow));
    }

    #[test]
    fn validate_ok() {
        let cfg = SonaConfig::new("/tmp", 64);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn builder_methods() {
        let cfg = SonaConfig::new("/tmp", 256)
            .with_replay_capacity(5000)
            .with_trajectory_window(50);
        assert_eq!(cfg.replay_capacity, 5000);
        assert_eq!(cfg.trajectory_window, 50);
    }
}

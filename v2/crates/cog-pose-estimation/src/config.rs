//! Runtime configuration for the pose-estimation Cog.
//!
//! Schema lives at `cog/config.schema.json` so the appliance can validate
//! before launching the cog.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CogConfig {
    /// URL of the local sensing-server's frame feed.
    /// Defaults to the appliance's loopback sensing-server.
    #[serde(default = "default_sensing_url")]
    pub sensing_url: String,

    /// Path to the model weights bundle (safetensors or HEF).
    /// Resolved relative to the cog's install dir if not absolute.
    pub model_path: PathBuf,

    /// Frame poll interval in milliseconds.
    #[serde(default = "default_poll_ms")]
    pub poll_ms: u64,

    /// Confidence threshold below which a frame's keypoints are not emitted.
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
}

fn default_sensing_url() -> String {
    "http://127.0.0.1:3000/api/v1/sensing/latest".to_string()
}

fn default_poll_ms() -> u64 {
    40 // ~25 Hz to match ESP32 CSI rate
}

fn default_min_confidence() -> f32 {
    0.3
}

impl CogConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
        let cfg: CogConfig =
            serde_json::from_str(&raw).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))?;
        Ok(cfg)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config at {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("failed to parse config at {0}: {1}")]
    Parse(PathBuf, serde_json::Error),
}
